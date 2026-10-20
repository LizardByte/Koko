use super::*;

const GROUP: &str = "f5093c06-23e3-404f-aeaa-40f72885ee3a";
const PERSON: &str = "46510d46-ce20-4dde-b9ea-207e474353f4";

fn group() -> Value {
    serde_json::json!({"id": GROUP, "name": "Group", "type": "Group", "relations": [
        {"type-id": MEMBERSHIP_ID, "direction": "backward", "begin": "1990", "end": "2000",
         "ended": true, "attributes": ["bass"],
         "artist": {"id": PERSON, "name": "Member", "type": "Person"}},
        {"type-id": MEMBERSHIP_ID, "direction": "backward", "begin": "2005",
         "ended": false, "attributes": ["bass", "vocals"],
         "artist": {"id": PERSON, "name": "Member", "type": "Person"}},
        {"type-id": "unrelated-relationship", "direction": "backward",
         "artist": {"id": GROUP, "name": "Producer"}},
        {"type-id": MEMBERSHIP_ID, "direction": "forward",
         "artist": {"id": GROUP, "name": "Parent group"}}
    ]})
}

#[tokio::test]
async fn members_keep_roles_dates_and_rejoining_periods() {
    let mut payload = group();
    enrich(&mut payload, false).await;
    let members: Vec<Member> = serde_json::from_value(payload["koko-members"].clone()).unwrap();
    let members_people = credits(&payload, &members);
    assert_eq!(members_people.len(), 1);
    assert_eq!(members_people[0].external_id.as_deref(), Some(PERSON));
    assert_eq!(members_people[0].department.as_deref(), Some("Members"));
    assert_eq!(
        members_people[0].role.as_deref(),
        Some("Member · bass, vocals · 1990–2000; 2005–present")
    );
    payload["relations"].as_array_mut().unwrap().remove(1);
    enrich(&mut payload, false).await;
    let members: Vec<Member> = serde_json::from_value(payload["koko-members"].clone()).unwrap();
    assert_eq!(
        people(&members)[0].role.as_deref(),
        Some("Former member · bass · 1990–2000")
    );
    payload["type"] = serde_json::json!("Person");
    assert!(super::members(&payload).is_empty());
    let solo = credits(&payload, &[]);
    assert_eq!(solo.len(), 1);
    assert_eq!(solo[0].external_id.as_deref(), Some(GROUP));
    assert_eq!(solo[0].department.as_deref(), Some("Artists"));
}

#[tokio::test]
async fn group_lookup_enriches_members_with_external_ids_without_merging_group_and_person_identity()
{
    use crate::metadata::{
        MetadataProvider,
        get_primary_item_metadata_link,
        upsert_item_metadata_snapshot,
    };
    use diesel::{
        Connection,
        RunQueryDsl,
        connection::SimpleConnection,
    };
    use std::io::{
        Read,
        Write,
    };
    use std::time::{
        Duration,
        Instant,
    };

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let root = format!("http://{}/ws/2", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        for (index, body) in [
            group(),
            serde_json::json!({
                "id": PERSON, "name": "Canonical member", "type": "Person", "gender": "Male",
                "life-span": {"begin": "1970-01-02"}, "begin-area": {"name": "Savannah"},
                "relations": [{"url": {"resource": "https://www.imdb.com/name/nm12345/"}}]
            }),
        ]
        .into_iter()
        .enumerate()
        {
            let deadline = Instant::now() + Duration::from_secs(30);
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(connection) => break connection,
                    Err(error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                            && Instant::now() < deadline =>
                    {
                        std::thread::sleep(Duration::from_millis(10))
                    }
                    Err(error) => panic!("Missing member lookup: {error}"),
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            let mut headers = Vec::new();
            while !headers.ends_with(b"\r\n\r\n") {
                let mut chunk = [0; 4096];
                let size = stream.read(&mut chunk).unwrap();
                assert!(size > 0 && headers.len() < 16384);
                headers.extend_from_slice(&chunk[..size]);
            }
            let request = String::from_utf8(headers).unwrap();
            let url = reqwest::Url::parse(&format!(
                "http://localhost{}",
                request.split_whitespace().nth(1).unwrap()
            ))
            .unwrap();
            let includes = url
                .query_pairs()
                .find(|(key, _)| key == "inc")
                .unwrap()
                .1
                .into_owned();
            assert_eq!(includes.contains("artist-rels"), index == 0);
            assert!(includes.contains("url-rels"));
            assert_eq!(
                url.path(),
                format!("/ws/2/artist/{}", if index == 0 { GROUP } else { PERSON })
            );
            let body = body.to_string();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: \
                 {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
        }
    });
    super::super::TEST_API_ROOT
        .scope(root, async {
            let snapshot = super::super::MusicBrainzMetadataProvider
                .fetch_snapshot(
                    &crate::config::MetadataSettings::default(),
                    GROUP,
                    "artist",
                    true,
                )
                .await
                .unwrap();
            let mut conn = diesel::SqliteConnection::establish(":memory:").unwrap();
            crate::db::run_pending_sqlite_migrations(&mut conn).unwrap();
            conn.batch_execute(
                "INSERT INTO media_libraries (id,name,path,kind) VALUES \
                 (1,'Music','music','music');
            INSERT INTO media_items (id,library_id,identity_key,item_type,display_title) VALUES \
                 (1,1,'group','artist','Group');
            INSERT INTO metadata_people (id,provider_id,external_id,name) VALUES \
                 (100,'tmdb','123','Screen member');
            INSERT INTO metadata_person_external_ids (person_id,source,external_id) VALUES \
                 (100,'imdb','nm12345');",
            )
            .unwrap();
            upsert_item_metadata_snapshot(&mut conn, 1, &snapshot).unwrap();
            assert!(
                crate::metadata::identities::for_artist(&mut conn, 1)
                    .unwrap()
                    .person_ids
                    .is_empty()
            );
            let peers = crate::metadata::identities::for_person(&mut conn, 100).unwrap();
            assert_eq!(peers.person_ids.len(), 2);
            assert!(peers.artist_ids.is_empty());
            use crate::db::schema::metadata_people::dsl as people;
            use diesel::{
                ExpressionMethods,
                QueryDsl,
            };
            let person = people::metadata_people
                .filter(people::provider_id.eq("musicbrainz"))
                .select((people::name, people::birthday, people::birth_place))
                .first::<(String, Option<String>, Option<String>)>(&mut conn)
                .unwrap();
            assert_eq!(
                person,
                (
                    "Canonical member".into(),
                    Some("1970-01-02".into()),
                    Some("Savannah".into())
                )
            );
            let credits = crate::metadata::list_metadata_person_credit_summaries_for_person_ids(
                &mut conn,
                &peers.person_ids,
            )
            .unwrap();
            assert_eq!(credits.len(), 1);
            assert_eq!(credits[0].department.as_deref(), Some("Members"));
            assert_eq!(
                get_primary_item_metadata_link(&mut conn, 1)
                    .unwrap()
                    .unwrap()
                    .refresh_state,
                "fresh"
            );
        })
        .await;
    server.join().unwrap();
}
