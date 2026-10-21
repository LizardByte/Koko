//! Incremental recommendation scores, independent of account access and page requests.

use super::*;
use diesel::{
    BoolExpressionMethods,
    Connection,
    Insertable,
};

#[derive(Insertable)]
#[diesel(table_name = cached)]
struct CachedMatch {
    item_id: i32,
    similar_item_id: i32,
    provider_priority: i32,
    provider_rank: i32,
    local_score: i32,
    year_distance: i64,
}

type Ranks = HashMap<Identity, i32>;

/// Update both incoming and outgoing matches when catalog facts change. All matches are retained
/// so access restrictions can be applied before the display limit, without rescoring on reads.
pub(crate) fn refresh(
    conn: &mut SqliteConnection,
    changed_ids: &[i32],
) -> QueryResult<()> {
    if changed_ids.is_empty() {
        return Ok(());
    }
    conn.transaction(|conn| {
        let (ids, affected) = catalog_scope(conn, changed_ids)?;
        let items: HashMap<_, _> = list_media_items_by_ids(conn, &ids, &[])?
            .into_iter()
            .map(|item| (item.id, item))
            .collect();
        let facts = load_facts(conn, &items)?;
        let effective: HashMap<_, _> = items
            .keys()
            .map(|id| (*id, inherited(*id, &items, &facts)))
            .collect();
        let ranks = provider_ranks(conn, &items)?;
        let mut all_by_type = HashMap::<_, Vec<_>>::new();
        let mut changed_by_type = HashMap::<_, Vec<_>>::new();
        for item in items.values() {
            all_by_type
                .entry(item.item_type.as_str())
                .or_default()
                .push(item);
            if affected.contains(&item.id) {
                changed_by_type
                    .entry(item.item_type.as_str())
                    .or_default()
                    .push(item);
            }
        }
        let mut matches = Vec::new();
        for seed in items.values() {
            let by_type = if affected.contains(&seed.id) { &all_by_type } else { &changed_by_type };
            let Some(candidates) = by_type.get(seed.item_type.as_str()) else {
                continue;
            };
            for candidate in candidates
                .iter()
                .filter(|candidate| candidate.id != seed.id)
            {
                if let Some(score) = score(seed, candidate, &facts, &effective, &ranks) {
                    matches.push(score);
                }
            }
        }
        let affected: Vec<_> = affected.into_iter().collect();
        for chunk in affected.chunks(400) {
            diesel::delete(
                cached::table.filter(
                    cached::item_id
                        .eq_any(chunk)
                        .or(cached::similar_item_id.eq_any(chunk)),
                ),
            )
            .execute(conn)?;
        }
        // Six bindings per match, below SQLite's conservative variable limit.
        for chunk in matches.chunks(128) {
            diesel::insert_into(cached::table)
                .values(chunk)
                .execute(conn)?;
        }
        Ok(())
    })
}

/// Keep cross-library candidates of the affected types, with their inheritance families. The
/// lightweight hierarchy query avoids hydrating unrelated music/books for a movie refresh.
fn catalog_scope(
    conn: &mut SqliteConnection,
    changed_ids: &[i32],
) -> QueryResult<(Vec<i32>, HashSet<i32>)> {
    let rows = item_rows::table
        .select((item_rows::id, item_rows::parent_id, item_rows::item_type))
        .load::<(i32, Option<i32>, String)>(conn)?;
    let parents: HashMap<_, _> = rows.iter().map(|(id, parent, _)| (*id, *parent)).collect();
    let ancestors = |id| {
        let mut ids = Vec::new();
        let mut current = Some(id);
        while let Some(id) = current {
            if ids.contains(&id) {
                break;
            }
            ids.push(id);
            current = parents.get(&id).copied().flatten();
        }
        ids
    };
    let family: HashSet<_> = changed_ids.iter().flat_map(|id| ancestors(*id)).collect();
    let mut affected = family.clone();
    for (id, _, _) in &rows {
        if ancestors(*id).iter().any(|id| family.contains(id)) {
            affected.insert(*id);
        }
    }
    let kinds: HashSet<_> = rows
        .iter()
        .filter(|(id, _, _)| affected.contains(id))
        .map(|(_, _, kind)| kind)
        .collect();
    let candidates: HashSet<_> = rows
        .iter()
        .filter(|(_, _, kind)| kinds.contains(kind))
        .flat_map(|(id, _, _)| ancestors(*id))
        .collect();
    let ids = rows
        .iter()
        .filter(|(id, _, _)| ancestors(*id).iter().any(|id| candidates.contains(id)))
        .map(|(id, _, _)| *id)
        .collect();
    Ok((ids, affected))
}

/// Resolve complete saved provider lists, including lists inherited from a parent artist or show.
fn provider_ranks(
    conn: &mut SqliteConnection,
    items: &HashMap<i32, MediaItemSummary>,
) -> QueryResult<HashMap<i32, Ranks>> {
    let mut own = HashMap::<i32, Ranks>::new();
    for (item_id, provider, kind, id, rank) in similar::table
        .inner_join(links::table)
        .filter(links::media_item_id.eq_any(items.keys().copied().collect::<Vec<_>>()))
        .filter(links::relation_kind.eq("primary"))
        .filter(links::match_state.eq("linked"))
        .select((
            links::media_item_id,
            similar::provider_id,
            similar::media_type,
            similar::external_id,
            similar::rank,
        ))
        .load::<(i32, String, String, String, i32)>(conn)?
    {
        own.entry(item_id)
            .or_default()
            .entry(Identity::new(&provider, &kind, &id))
            .and_modify(|current| *current = (*current).min(rank))
            .or_insert(rank);
    }
    Ok(items
        .keys()
        .map(|id| {
            let mut ranks = Ranks::new();
            for source in lineage(*id, items) {
                if let Some(saved) = own.get(&source) {
                    for (identity, rank) in saved {
                        ranks
                            .entry(identity.clone())
                            .and_modify(|current| *current = (*current).min(*rank))
                            .or_insert(*rank);
                    }
                }
            }
            (*id, ranks)
        })
        .collect())
}

fn score(
    seed: &MediaItemSummary,
    candidate: &MediaItemSummary,
    facts: &HashMap<i32, Facts>,
    effective: &HashMap<i32, Facts>,
    ranks: &HashMap<i32, Ranks>,
) -> Option<CachedMatch> {
    let own = &facts[&candidate.id];
    if !own.identities.is_disjoint(&facts[&seed.id].identities) {
        return None;
    }
    let seed_facts = &effective[&seed.id];
    let candidate_facts = &effective[&candidate.id];
    let ranks = &ranks[&seed.id];
    let direct_ids = if ["season", "episode"].contains(&candidate.item_type.as_str()) {
        &candidate_facts.identities
    } else {
        &own.identities
    };
    let direct = direct_ids
        .iter()
        .filter_map(|key| ranks.get(key))
        .min()
        .copied();
    let artist = candidate_facts
        .artists
        .iter()
        .filter_map(|key| ranks.get(key))
        .min()
        .copied();
    let (provider_priority, provider_rank) = direct
        .map(|rank| (2, rank))
        .or_else(|| artist.map(|rank| (1, rank)))
        .unwrap_or_default();
    let local_score = (candidate_facts
        .genres
        .intersection(&seed_facts.genres)
        .count()
        * 10
        + candidate_facts
            .creators
            .intersection(&seed_facts.creators)
            .count()
            * 30
        + candidate_facts
            .collections
            .intersection(&seed_facts.collections)
            .count()
            * 40) as i32;
    if provider_priority == 0 && local_score == 0 {
        return None;
    }
    Some(CachedMatch {
        item_id: seed.id,
        similar_item_id: candidate.id,
        provider_priority,
        provider_rank,
        local_score,
        year_distance: seed
            .release_year
            .zip(candidate.release_year)
            .map(|(left, right)| (i64::from(left) - i64::from(right)).abs())
            .unwrap_or(i64::MAX),
    })
}
