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
        // Take the writer lock before reading facts. Promoting a deferred WAL read transaction
        // after another connection commits would fail immediately with SQLITE_BUSY_SNAPSHOT.
        clear_matches(conn, changed_ids)?;
        // Excluded item refreshes must not rebuild their entire show or artist family.
        let mut seeds = Vec::new();
        for chunk in changed_ids.chunks(500) {
            seeds.extend(
                item_rows::table
                    .filter(item_rows::id.eq_any(chunk))
                    .filter(item_rows::item_type.ne_all(EXCLUDED_ITEM_TYPES))
                    .select(item_rows::id)
                    .load::<i32>(conn)?,
            );
        }
        if seeds.is_empty() {
            return Ok(());
        }
        let (ids, affected) = catalog_scope(conn, &seeds)?;
        let items: HashMap<_, _> = list_media_items_by_ids(conn, &ids, &[])?
            .into_iter()
            .map(|item| (item.id, item))
            .collect();
        let facts = load_facts(conn, &items)?;
        let eligible: Vec<_> = items
            .values()
            .filter(|item| !EXCLUDED_ITEM_TYPES.contains(&item.item_type.as_str()))
            .collect();
        let effective: HashMap<_, _> = eligible
            .iter()
            .map(|item| (item.id, inherited(item.id, &items, &facts)))
            .collect();
        let ranks = provider_ranks(conn, &items)?;
        let mut all_by_type = HashMap::<_, Vec<_>>::new();
        let mut changed_by_type = HashMap::<_, Vec<_>>::new();
        for item in eligible.iter().copied() {
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
        for seed in eligible {
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
        let already_cleared: HashSet<_> = changed_ids.iter().copied().collect();
        let affected: Vec<_> = affected.difference(&already_cleared).copied().collect();
        clear_matches(conn, &affected)?;
        // Six bindings per match, below SQLite's conservative variable limit.
        for chunk in matches.chunks(128) {
            diesel::insert_into(cached::table)
                .values(chunk)
                .execute(conn)?;
        }
        Ok(())
    })
}

/// Remove old incoming and outgoing matches, including obsolete results for excluded item types.
fn clear_matches(
    conn: &mut SqliteConnection,
    ids: &[i32],
) -> QueryResult<()> {
    for chunk in ids.chunks(400) {
        diesel::delete(
            cached::table.filter(
                cached::item_id
                    .eq_any(chunk)
                    .or(cached::similar_item_id.eq_any(chunk)),
            ),
        )
        .execute(conn)?;
    }
    Ok(())
}

/// Keep cross-library candidates of the affected types, with their inheritance families. The
/// lightweight hierarchy excludes TV children; tracks provide genres for their music parents.
fn catalog_scope(
    conn: &mut SqliteConnection,
    changed_ids: &[i32],
) -> QueryResult<(Vec<i32>, HashSet<i32>)> {
    let rows = item_rows::table
        .filter(item_rows::item_type.ne_all(EXCLUDED_FACT_TYPES))
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
        .filter(|(id, _, kind)| {
            affected.contains(id) && !EXCLUDED_ITEM_TYPES.contains(&kind.as_str())
        })
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

/// Resolve complete saved provider lists, including lists inherited from a parent artist.
fn provider_ranks(
    conn: &mut SqliteConnection,
    items: &HashMap<i32, MediaItemSummary>,
) -> QueryResult<HashMap<i32, Ranks>> {
    let ids: Vec<_> = items
        .values()
        .filter(|item| !EXCLUDED_ITEM_TYPES.contains(&item.item_type.as_str()))
        .map(|item| item.id)
        .collect();
    let mut own = HashMap::<i32, Ranks>::new();
    for (item_id, provider, kind, id, rank) in similar::table
        .inner_join(links::table)
        .filter(links::media_item_id.eq_any(&ids))
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
    Ok(ids
        .iter()
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
    let direct = own
        .identities
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
