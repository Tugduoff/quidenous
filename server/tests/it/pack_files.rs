//! Pack files (docs/PACK_FILES.md): headers, choose questions in packs, admins without a pack limit,
//! `seed-packs`, the community packs in server/packs and the AI template.

use crate::common;
use quidenous_server::platform::ids::UserId;

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use quidenous_server::{
    AppState, app,
    community::packfile,
    community::packs::{self, SeedAction},
    platform::config::Config,
    platform::i18n::Lang,
    state,
};
use serde_json::{Value, json};
use tower::ServiceExt;

/// A fresh server; the first account to join (user 1) is the admin.
async fn setup() -> (Router, AppState) {
    let dir = common::temp_dir("qdn-packfiles");
    let config = Config {
        admin_user_ids: vec![UserId::from_raw(1)],
        ..common::config(&dir)
    };
    let state = state(config).await.unwrap();
    (app(state.clone()), state)
}

async fn get_text(app: &Router, path: &str) -> (StatusCode, String, String) {
    let res = app
        .clone()
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let kind = res
        .headers()
        .get("content-type")
        .map(|v| v.to_str().unwrap().to_owned())
        .unwrap_or_default();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (status, kind, String::from_utf8_lossy(&bytes).into_owned())
}

const FILE: &str = "# titre: Soirée coloc\n\
# description: Le frigo, la vaisselle et les voisins.\n\
# catégorie: Entre potes, Soirée\n\
# langue: fr\n\
# 16+: non\n\
# taille: small\n\
\n\
Qui laisse la vaisselle tremper quatre jours ?\n\
Qui mange le fromage des autres en douce ?\n\
# un commentaire au milieu\n\
[classement] Classe-nous du plus bruyant au plus discret.\n\
[choix] Ta place idéale dans la coloc ? | La grande chambre | Le canapé | Près de la cuisine\n\
[choix] Une réponse ne suffit pas ici ? | Seule\n\
Qui oublie toujours de racheter du papier toilette ?\n\
Qui invite des gens sans prévenir personne ?\n";

#[tokio::test]
async fn a_pack_file_carries_its_header_and_choose_questions() {
    let (app, state) = setup().await;
    let (_admin, _) = common::join(&app, "Admin").await;
    let (ana, _) = common::join(&app, "Ana").await;
    common::trust_all(&state).await;

    let (s, f) = common::call(&app, "GET", "/features", None, None, None).await;
    assert_eq!(s, StatusCode::OK);
    for flag in ["pack_choose", "pack_template"] {
        assert!(f.as_array().unwrap().contains(&json!(flag)), "{flag}");
    }

    // The header fills what the body leaves out; the bad choose line is left out.
    let (s, b) = common::call(
        &app,
        "POST",
        "/packs",
        Some(&ana),
        None,
        Some(json!({ "text": FILE })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["count"], 6);
    let id = b["id"].as_i64().unwrap();
    let (_, p) = common::call(&app, "GET", &format!("/packs/{id}"), Some(&ana), None, None).await;
    assert_eq!(p["name"], "Soirée coloc");
    assert_eq!(p["description"], "Le frigo, la vaisselle et les voisins.");
    assert_eq!(p["tags"], json!(["potes", "soiree"]));
    assert_eq!(p["language"], "fr");
    assert_eq!(p["spicy"], false);
    let qs = p["questions"].as_array().unwrap();
    let kinds: Vec<&str> = qs.iter().map(|q| q["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["pick", "pick", "rank", "choose", "pick", "pick"]);
    assert_eq!(qs[3]["text"], "Ta place idéale dans la coloc ?");
    assert_eq!(
        qs[3]["items"],
        json!(["La grande chambre", "Le canapé", "Près de la cuisine"])
    );
    assert_eq!(qs[0]["items"], Value::Null);

    // The body wins over the header; a pimente category forces 16+.
    let spicy = FILE.replace("Entre potes, Soirée", "pimente");
    let (s, b) = common::call(
        &app,
        "POST",
        "/packs",
        Some(&ana),
        None,
        Some(json!({ "text": spicy, "name": "Autre titre", "language": "en" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let (_, p2) = common::call(
        &app,
        "GET",
        &format!("/packs/{}", b["id"]),
        Some(&ana),
        None,
        None,
    )
    .await;
    assert_eq!(
        (p2["name"].as_str(), p2["language"].as_str()),
        (Some("Autre titre"), Some("en"))
    );
    assert_eq!(
        (p2["tags"].clone(), p2["spicy"].clone()),
        (json!(["pimente"]), json!(true))
    );

    // From a list: choose questions with valid answers stay, others are left out.
    let mut list: Vec<Value> = (0..5)
        .map(|i| json!({ "text": format!("Qui de nous ferait le numéro {i} ?"), "kind": "pick" }))
        .collect();
    list.push(json!({ "text": "Le dîner parfait pour toi ?", "kind": "choose", "items": ["Pizza", "Sushi"] }));
    list.push(json!({ "text": "Sans ses réponses du tout ?", "kind": "choose" }));
    list.push(json!({ "text": "Avec trop de réponses ici ?", "kind": "choose", "items": ["A", "B", "C", "D", "E"] }));
    let (s, b) = common::call(
        &app,
        "POST",
        "/packs",
        Some(&ana),
        None,
        Some(json!({ "name": "Liste", "questions": list })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["count"], 6);
    let listed = b["id"].as_i64().unwrap();

    // An app from before `pack_choose` edits the pack: its choose question comes back without
    // answers and keeps the stored ones.
    let (_, p) = common::call(&app, "GET", &format!("/packs/{id}"), Some(&ana), None, None).await;
    let old_app: Vec<Value> = p["questions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|q| json!({ "text": q["text"], "kind": q["kind"] }))
        .collect();
    let (s, e) = common::call(
        &app,
        "PUT",
        &format!("/packs/{id}"),
        Some(&ana),
        None,
        Some(json!({ "name": "Soirée coloc", "questions": old_app })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{e}");
    assert_eq!(e["version"], 1, "nothing changed");
    let (_, p) = common::call(&app, "GET", &format!("/packs/{id}"), Some(&ana), None, None).await;
    assert_eq!(
        p["questions"][3]["items"],
        json!(["La grande chambre", "Le canapé", "Près de la cuisine"])
    );

    // Installed into a room, they are real choose questions.
    let room = common::room_id(&app, &ana).await;
    let (s, b) = common::call(
        &app,
        "POST",
        &format!("/packs/{id}/install"),
        Some(&ana),
        Some(room),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["added"], 6);
    let (kind, items): (String, Option<String>) = sqlx::query_as(
        "SELECT kind, items FROM questions WHERE group_id = ? AND text = 'Ta place idéale dans la coloc ?'",
    )
    .bind(room)
    .fetch_one(&state.db)
    .await
    .unwrap();
    assert_eq!(kind, "choose");
    assert_eq!(
        serde_json::from_str::<Vec<String>>(&items.unwrap()).unwrap(),
        ["La grande chambre", "Le canapé", "Près de la cuisine"]
    );
    let (s, _) = common::call(
        &app,
        "POST",
        &format!("/packs/{listed}/install"),
        Some(&ana),
        Some(room),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM questions WHERE group_id = ? AND kind = 'choose' AND items IS NOT NULL")
        .bind(room)
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(n, 2);

    // A pack published from the room's own questions keeps its choose questions.
    let (s, b) = common::call(
        &app,
        "POST",
        "/packs",
        Some(&ana),
        Some(room),
        Some(json!({ "name": "Du salon" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let (_, p) = common::call(
        &app,
        "GET",
        &format!("/packs/{}", b["id"]),
        Some(&ana),
        None,
        None,
    )
    .await;
    let chooses = p["questions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|q| q["kind"] == "choose" && q["items"].is_array())
        .count();
    assert_eq!(chooses, 2);

    // The room import reads the same file and skips the header.
    let (_, room2) = common::call(
        &app,
        "POST",
        "/rooms",
        Some(&ana),
        None,
        Some(json!({ "name": "Deux" })),
    )
    .await;
    let room2 = room2["id"].as_i64().unwrap();
    let (s, b) = common::call(
        &app,
        "POST",
        "/questions/import",
        Some(&ana),
        Some(room2),
        Some(json!({ "text": FILE })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["added"], 6);
    let titles: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM questions WHERE group_id = ? AND text LIKE '%titre%'",
    )
    .bind(room2)
    .fetch_one(&state.db)
    .await
    .unwrap();
    assert_eq!(titles, 0);
}

#[tokio::test]
async fn admins_have_no_pack_limit_and_publish_into_the_selection() {
    let (app, state) = setup().await;
    let (admin, _) = common::join(&app, "Admin").await;
    let (ana, _) = common::join(&app, "Ana").await;
    common::trust_all(&state).await;
    let questions = |p: &str| -> Vec<Value> {
        (0..5)
            .map(|i| json!({ "text": format!("{p} question numéro {i} ?"), "kind": "pick" }))
            .collect()
    };
    for i in 0..11 {
        let (s, b) = common::call(
            &app,
            "POST",
            "/packs",
            Some(&admin),
            None,
            Some(json!({ "name": format!("Admin {i}"), "questions": questions("a"), "featured": i == 0 })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "admin pack {i}: {b}");
        assert_eq!(b["featured"], i == 0);
    }
    let (_, featured) =
        common::call(&app, "GET", "/packs?sort=featured", Some(&ana), None, None).await;
    let names: Vec<&str> = featured
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Admin 0"]);

    // Others: `featured` is ignored, and the limit holds.
    for i in 0..10 {
        let (s, b) = common::call(
            &app,
            "POST",
            "/packs",
            Some(&ana),
            None,
            Some(json!({ "name": format!("Ana {i}"), "questions": questions("b"), "featured": true })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
        assert_eq!(b["featured"], false);
    }
    let (s, b) = common::call(
        &app,
        "POST",
        "/packs",
        Some(&ana),
        None,
        Some(json!({ "name": "Ana 10", "questions": questions("b") })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("too_many_packs"))
    );
}

#[tokio::test]
async fn seed_packs_publishes_a_folder_once() {
    let (app, state) = setup().await;
    let (tug, _) = common::join(&app, "Tug").await;
    let author: i64 = sqlx::query_scalar("SELECT id FROM users WHERE name = 'Tug'")
        .fetch_one(&state.db)
        .await
        .unwrap();
    let dir = common::temp_dir("qdn-seed");
    std::fs::create_dir_all(dir.join("potes")).unwrap();
    std::fs::write(dir.join("potes/coloc.fr.txt"), FILE).unwrap();
    std::fs::write(
        dir.join("potes/coloc.en.txt"),
        "# title: Flat life\n# category: friends\n# lang: en\n# 16+: no\n\
         Who leaves the pan soaking?\nWho eats everyone's cheese?\nWho never buys loo roll?\n\
         [rank] Rank us from loudest to quietest.\n[choose] Your spot in the flat? | The sofa | The kitchen\n",
    )
    .unwrap();
    std::fs::write(dir.join("broken.txt"), "# titre: Sans langue\nQui ?\n").unwrap();
    std::fs::write(dir.join("notes.md"), "# titre: pas un pack\n").unwrap();

    let names = |o: &[packs::SeedOutcome]| -> Vec<String> {
        o.iter()
            .map(|o| o.path.strip_prefix(&dir).unwrap().display().to_string())
            .collect()
    };
    let dry = packs::seed_dir(&state.db, &dir, UserId::from_raw(author), true, true)
        .await
        .unwrap();
    assert_eq!(
        names(&dry),
        ["broken.txt", "potes/coloc.en.txt", "potes/coloc.fr.txt"]
    );
    assert!(matches!(dry[0].action, SeedAction::Invalid(_)));
    assert_eq!(
        dry[1].action,
        SeedAction::Created {
            id: None,
            count: 5,
            dropped: 0
        }
    );
    assert_eq!(
        dry[2].action,
        SeedAction::Created {
            id: None,
            count: 6,
            dropped: 1
        }
    );
    let none: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM packs")
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(none, 0, "a dry run writes nothing");

    let real = packs::seed_dir(&state.db, &dir, UserId::from_raw(author), true, false)
        .await
        .unwrap();
    let SeedAction::Created { id: Some(en), .. } = real[1].action else {
        panic!("{:?}", real[1])
    };
    let SeedAction::Created { id: Some(fr), .. } = real[2].action else {
        panic!("{:?}", real[2])
    };
    let (_, p) = common::call(&app, "GET", &format!("/packs/{fr}"), Some(&tug), None, None).await;
    assert_eq!(
        (
            p["name"].as_str(),
            p["featured"].as_bool(),
            p["status"].as_str()
        ),
        (Some("Soirée coloc"), Some(true), Some("public"))
    );
    assert_eq!(p["tags"], json!(["potes", "soiree"]));
    let (_, p) = common::call(&app, "GET", &format!("/packs/{en}"), Some(&tug), None, None).await;
    assert_eq!(
        (p["language"].as_str(), p["count"].as_i64()),
        (Some("en"), Some(5))
    );
    assert_eq!(
        p["questions"][4]["items"],
        json!(["The sofa", "The kitchen"])
    );

    // Again: nothing new.
    let again = packs::seed_dir(&state.db, &dir, UserId::from_raw(author), true, false)
        .await
        .unwrap();
    assert_eq!(again[1].action, SeedAction::Exists { id: en });
    assert_eq!(again[2].action, SeedAction::Exists { id: fr });
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM packs")
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(total, 2);

    // A deleted pack doesn't count: seeding publishes it again.
    sqlx::query("UPDATE packs SET status = 'deleted' WHERE id = ?")
        .bind(en)
        .execute(&state.db)
        .await
        .unwrap();
    let after = packs::seed_dir(&state.db, &dir, UserId::from_raw(author), false, false)
        .await
        .unwrap();
    assert!(matches!(
        after[1].action,
        SeedAction::Created { id: Some(_), .. }
    ));

    assert!(
        packs::seed_dir(&state.db, &dir, UserId::from_raw(999), false, true)
            .await
            .is_err()
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// Every community pack in server/packs is a complete, valid pack file with nothing left out, and
/// each French pack has its English twin with as many questions of each kind.
#[test]
fn the_community_packs_are_valid() {
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|e| e == "txt") {
                out.push(p);
            }
        }
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("packs");
    let mut files = Vec::new();
    walk(&root, &mut files);
    files.sort();
    assert_eq!(files.len(), 84);
    let mut kinds = std::collections::HashMap::new();
    let mut titles = std::collections::HashSet::new();
    for f in &files {
        let content = std::fs::read_to_string(f).unwrap();
        let (pack, dropped) = packs::pack_from_file(&content, true)
            .unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        assert_eq!(dropped, 0, "{}", f.display());
        let header = packfile::parse(&content).header;
        assert!(
            header.description.is_some() && !header.categories.is_empty() && header.spicy.is_some(),
            "{}",
            f.display()
        );
        assert_eq!(
            header.spicy,
            Some(header.categories.contains(&"pimente")),
            "16+ exactly on pimente: {}",
            f.display()
        );
        let lang = if f.to_string_lossy().ends_with(".fr.txt") {
            Lang::Fr
        } else {
            Lang::En
        };
        assert_eq!(pack.language, lang, "{}", f.display());
        assert!(
            titles.insert((pack.name.to_lowercase(), lang)),
            "duplicate title {}",
            pack.name
        );
        let mix: Vec<String> = pack.questions.iter().map(|q| q.kind.clone()).collect();
        let stem = f
            .file_name()
            .unwrap()
            .to_string_lossy()
            .split('.')
            .next()
            .unwrap()
            .to_owned();
        kinds.entry(stem).or_insert_with(Vec::new).push(mix);
    }
    for (stem, mixes) in kinds {
        assert_eq!(mixes.len(), 2, "{stem}: FR and EN");
        assert_eq!(mixes[0], mixes[1], "{stem}: same kinds in both languages");
    }
}

/// The AI template's example files are valid pack files, and both languages are served.
#[tokio::test]
async fn the_ai_template_is_served_and_its_examples_parse() {
    for (lang, template) in packfile::TEMPLATES {
        let examples: Vec<&str> = template
            .split("```text\n")
            .skip(1)
            .map(|b| b.split("```").next().unwrap())
            .collect();
        assert_eq!(examples.len(), 2, "{lang:?}");
        for example in examples {
            let file = packfile::parse(example);
            assert_eq!(file.header.lang, Some(lang));
            assert!(
                file.header.title.is_some()
                    && file.header.description.is_some()
                    && file.header.spicy.is_some()
            );
            assert!(!file.header.categories.is_empty());
            let (pack, dropped) = packs::pack_from_file(example, false).unwrap();
            assert_eq!(dropped, 0);
            assert_eq!(
                pack.questions.iter().filter(|q| q.kind == "choose").count(),
                1
            );
            assert!(pack.questions.iter().any(|q| q.kind == "rank"));
        }
    }
    let (app, _) = setup().await;
    let (s, kind, fr) = get_text(&app, "/pack-template.md").await;
    assert_eq!(s, StatusCode::OK);
    assert!(kind.starts_with("text/markdown"));
    assert!(fr.contains("# titre:"));
    let (s, _, en) = get_text(&app, "/en/pack-template.md").await;
    assert_eq!(s, StatusCode::OK);
    assert!(en.contains("# title:"));
}
