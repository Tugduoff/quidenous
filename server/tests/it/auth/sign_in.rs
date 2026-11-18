use super::*;

#[tokio::test]
async fn sign_up_verify_sign_in_and_join_a_room() {
    let t = setup().await;
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/email",
        None,
        Some(json!({ "email": "ana@example.com" })),
    )
    .await;
    assert_eq!((s, &b["account"]), (StatusCode::OK, &json!(false)));
    for (email, password, name, error) in [
        ("nope", PASSWORD, "Ana", "bad_email"),
        ("ana@example.com", PASSWORD, " ", "bad_name"),
        ("ana@example.com", "short", "Ana", "weak_password"),
        ("ana@example.com", "password123", "Ana", "common_password"),
        ("ana@example.com", "ana@example.com", "Ana", "weak_password"),
    ] {
        let (s, b) = call(
            &t.app,
            "POST",
            "/auth/signup",
            None,
            Some(json!({ "email": email, "password": password, "name": name })),
        )
        .await;
        assert_eq!(
            (s, b["error"].as_str()),
            (StatusCode::BAD_REQUEST, Some(error))
        );
    }
    let r = send(
        &t.app,
        "POST",
        "/auth/signup",
        None,
        Some(json!({ "email": " Ana@Example.com", "password": PASSWORD, "name": "Ana" })),
        &[("x-device", "iPhone%20%C2%B7%20iOS%2018")],
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.body["email_sent"], true);
    let token = r.body["token"].as_str().unwrap().to_owned();
    assert_eq!(t.mail.count("ana@example.com"), 1);
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/signup",
        None,
        Some(json!({ "email": "ana@example.com", "password": PASSWORD, "name": "Ana" })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("email_taken"))
    );
    let (_, b) = call(
        &t.app,
        "POST",
        "/auth/email",
        None,
        Some(json!({ "email": "ANA@example.com" })),
    )
    .await;
    assert_eq!(b["account"], true);

    let (_, acct) = call(&t.app, "GET", "/auth/account", Some(&token), None).await;
    assert_eq!(acct["email"], "ana@example.com");
    assert_eq!(acct["email_verified"], false);
    assert_eq!(acct["has_password"], true);
    assert_eq!(acct["secured"], true);
    assert_eq!(acct["totp"], false);

    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/verify",
        Some(&token),
        Some(json!({ "code": "000000" })),
    )
    .await;
    if t.mail.last_code("ana@example.com").as_deref() != Some("000000") {
        assert_eq!(
            (s, b["error"].as_str()),
            (StatusCode::BAD_REQUEST, Some("bad_code"))
        );
    }
    let old = t.mail.last_code("ana@example.com").unwrap();
    let (s, b) = call(&t.app, "POST", "/auth/verify/send", Some(&token), None).await;
    assert_eq!((s, &b["email_sent"]), (StatusCode::OK, &json!(true)));
    let new = t.mail.last_code("ana@example.com").unwrap();
    if old != new {
        let (s, _) = call(
            &t.app,
            "POST",
            "/auth/verify",
            Some(&token),
            Some(json!({ "code": old })),
        )
        .await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
    }
    verify_email(&t, &token, "ana@example.com").await;
    let (s, b) = call(&t.app, "POST", "/auth/verify/send", Some(&token), None).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("already_verified"))
    );

    let (s, b) = call(&t.app, "GET", "/me", Some(&token), None).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("no_room"))
    );
    let (s, room) = call(
        &t.app,
        "POST",
        "/rooms",
        Some(&token),
        Some(json!({ "name": "Les potes" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert!(room["id"].as_i64().is_some());
    let (s, me) = call(&t.app, "GET", "/me", Some(&token), None).await;
    assert_eq!((s, me["name"].as_str()), (StatusCode::OK, Some("Ana")));

    let (s1, b1) = login(&t, "ana@example.com", "wrong-password-1").await;
    let (s2, b2) = login(&t, "nobody@example.com", PASSWORD).await;
    assert_eq!(
        (s1, &b1),
        (
            StatusCode::BAD_REQUEST,
            &json!({ "error": "bad_credentials" })
        )
    );
    assert_eq!((s2, &b2), (s1, &b1));
    let (s, b) = login(&t, "ANA@example.com", PASSWORD).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["new"], false);
    let second = b["token"].as_str().unwrap();
    let (s, _) = call(&t.app, "GET", "/me", Some(second), None).await;
    assert_eq!(s, StatusCode::OK);

    let (_, sessions) = call(&t.app, "GET", "/auth/sessions", Some(&token), None).await;
    let mine = sessions
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["current"] == true)
        .unwrap();
    assert_eq!(mine["device"], "iPhone · iOS 18");
    assert!(mine["created_at"].as_str().unwrap().contains('T'));
}

#[tokio::test]
async fn failed_sign_ins_lock_the_account_and_ips_are_limited() {
    let t = setup().await;
    signup(&t, "bob@example.com", "Bob").await;
    for _ in 0..5 {
        let (s, _) = login(&t, "bob@example.com", "not-the-password").await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
    }
    let r = send(
        &t.app,
        "POST",
        "/auth/login",
        None,
        Some(json!({ "email": "bob@example.com", "password": PASSWORD })),
        &[],
    )
    .await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(r.body["error"], "rate_limited");
    let wait = r.body["retry_after"].as_u64().unwrap();
    assert!((1..=180).contains(&wait), "{wait}");
    assert!(r.headers.get("retry-after").is_some());

    for i in 0..quidenous_server::accounts::limits::LOGIN_IP.capacity {
        let forwarded = format!("198.18.0.{i}, 203.0.113.7");
        let r = send(
            &t.app,
            "POST",
            "/auth/login",
            None,
            Some(json!({ "email": format!("x{i}@example.com"), "password": "whatever-123" })),
            &[("x-forwarded-for", &forwarded)],
        )
        .await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST);
    }
    let r = send(
        &t.app,
        "POST",
        "/auth/login",
        None,
        Some(json!({ "email": "x@example.com", "password": "whatever-123" })),
        &[("x-forwarded-for", "203.0.113.7")],
    )
    .await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS);
    let r = send(
        &t.app,
        "POST",
        "/auth/login",
        None,
        Some(json!({ "email": "x@example.com", "password": "whatever-123" })),
        &[("x-forwarded-for", "203.0.113.8")],
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);

    for i in 0..10 {
        let r = send(
            &t.app,
            "POST",
            "/join",
            None,
            Some(json!({ "code": "code-1", "name": format!("P{i}") })),
            &[("x-forwarded-for", "198.51.100.1")],
        )
        .await;
        assert_eq!(r.status, StatusCode::OK);
    }
    let r = send(
        &t.app,
        "POST",
        "/start",
        None,
        Some(json!({ "name": "Zoe", "room": "Chez Zoe" })),
        &[("x-forwarded-for", "198.51.100.1")],
    )
    .await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn two_factor_setup_sign_in_recovery_and_disable() {
    let t = setup().await;
    let (token, user) = signup(&t, "cyd@example.com", "Cyd").await;
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/2fa/enable",
        Some(&token),
        Some(json!({ "code": "123456" })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("reauth_required"))
    );
    let (s, b) = call(&t.app, "POST", "/auth/2fa/setup", Some(&token), None).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("reauth_required"))
    );
    reauth(&t, &token).await;
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/2fa/enable",
        Some(&token),
        Some(json!({ "code": "123456" })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("no_setup"))
    );
    let (s, setup) = call(&t.app, "POST", "/auth/2fa/setup", Some(&token), None).await;
    assert_eq!(s, StatusCode::OK);
    let secret = setup["secret"].as_str().unwrap().to_owned();
    let url = setup["otpauth_url"].as_str().unwrap();
    assert!(url.starts_with("otpauth://totp/Qui%20de%20nous%20%3F:cyd%40example.com?secret="));
    assert!(url.contains("issuer=Qui%20de%20nous%20%3F"));

    let (_, other) = login(&t, "cyd@example.com", PASSWORD).await;
    let other = other["token"].as_str().unwrap().to_owned();

    let wrong = if totp_code(&secret, now()).unwrap() == "000000" {
        "111111"
    } else {
        "000000"
    };
    let (s, _) = call(
        &t.app,
        "POST",
        "/auth/2fa/enable",
        Some(&token),
        Some(json!({ "code": wrong })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let used = totp_code(&secret, now()).unwrap();
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/2fa/enable",
        Some(&token),
        Some(json!({ "code": used, "revoke_others": true })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["revoked"], 1);
    let recovery: Vec<String> = b["recovery_codes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c.as_str().unwrap().to_owned())
        .collect();
    assert_eq!(recovery.len(), 10);
    let (s, _) = call(&t.app, "GET", "/auth/account", Some(&other), None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (_, acct) = call(&t.app, "GET", "/auth/account", Some(&token), None).await;
    assert_eq!(
        (&acct["totp"], &acct["recovery_codes_left"]),
        (&json!(true), &json!(10))
    );

    let (s, b) = login(&t, "cyd@example.com", PASSWORD).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(b["mfa"], "totp");
    assert!(b.get("token").is_none());
    let challenge = b["challenge"].as_str().unwrap().to_owned();
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/login/2fa",
        None,
        Some(json!({ "challenge": challenge, "code": used })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_code"))
    );
    let next = totp_code(&secret, now() + 30).unwrap();
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/login/2fa",
        None,
        Some(json!({ "challenge": challenge, "code": next })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["user_id"], user);
    let (s, _) = call(
        &t.app,
        "POST",
        "/auth/login/2fa",
        None,
        Some(json!({ "challenge": challenge, "code": next })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);

    for expected in [StatusCode::OK, StatusCode::BAD_REQUEST] {
        let (_, b) = login(&t, "cyd@example.com", PASSWORD).await;
        let (s, _) = call(
            &t.app,
            "POST",
            "/auth/login/2fa",
            None,
            Some(json!({ "challenge": b["challenge"], "code": recovery[0].to_uppercase() })),
        )
        .await;
        assert_eq!(s, expected);
    }
    let (_, acct) = call(&t.app, "GET", "/auth/account", Some(&token), None).await;
    assert_eq!(acct["recovery_codes_left"], 9);

    let (_, b) = login(&t, "cyd@example.com", PASSWORD).await;
    let challenge = b["challenge"].as_str().unwrap().to_owned();
    for _ in 0..5 {
        call(
            &t.app,
            "POST",
            "/auth/login/2fa",
            None,
            Some(json!({ "challenge": challenge, "code": "aaaaa-bbbbb" })),
        )
        .await;
    }
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/login/2fa",
        None,
        Some(json!({ "challenge": challenge, "code": recovery[1] })),
    )
    .await;
    assert!(
        s == StatusCode::TOO_MANY_REQUESTS || b["error"] == "expired",
        "{s} {b}"
    );

    sqlx::query("UPDATE users SET totp_last_step = NULL")
        .execute(&t.state.db)
        .await
        .unwrap();
    let config = &t.state.config;
    let fresh = app(state_with_mailer(
        Config {
            database_url: config.database_url.clone(),
            data_dir: config.data_dir.clone(),
            auth_secret: config.auth_secret.clone(),
            group_name: "Test".into(),
            join_code: "CODE-1".into(),
            ..Config::default()
        },
        t.mail.clone(),
    )
    .await
    .unwrap());
    let (s, b) = call(
        &fresh,
        "POST",
        "/auth/2fa/recovery",
        Some(&token),
        Some(json!({ "code": recovery[2] })),
    )
    .await;
    assert_eq!(
        s,
        StatusCode::BAD_REQUEST,
        "recovery codes can't make new ones: {b}"
    );
    let (s, b) = call(
        &fresh,
        "POST",
        "/auth/2fa/recovery",
        Some(&token),
        Some(json!({ "code": totp_code(&secret, now()).unwrap() })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let renewed: Vec<String> = serde_json::from_value(b["recovery_codes"].clone()).unwrap();
    let (s, _) = call(
        &fresh,
        "POST",
        "/auth/2fa/disable",
        Some(&token),
        Some(json!({ "code": recovery[3] })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "old recovery codes are gone");
    let (s, b) = call(
        &fresh,
        "POST",
        "/auth/2fa/disable",
        Some(&token),
        Some(json!({ "code": renewed[0] })),
    )
    .await;
    assert_eq!((s, &b["ok"]), (StatusCode::OK, &json!(true)));
    let (s, b) = login(&t, "cyd@example.com", PASSWORD).await;
    assert_eq!(s, StatusCode::OK);
    assert!(b["token"].is_string());
}

#[tokio::test]
async fn forgot_password_resets_and_signs_out_everywhere() {
    let t = setup().await;
    let (first, _) = signup(&t, "dan@example.com", "Dan").await;
    let (_, b) = login(&t, "dan@example.com", PASSWORD).await;
    let second = b["token"].as_str().unwrap().to_owned();
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/forgot",
        None,
        Some(json!({ "email": "ghost@example.com" })),
    )
    .await;
    assert_eq!((s, &b["sent"]), (StatusCode::OK, &json!(true)));
    let (s, _) = call(
        &t.app,
        "POST",
        "/auth/forgot",
        None,
        Some(json!({ "email": "dan@example.com" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let code = wait_for_code(&t.mail, "dan@example.com", 2).await;
    let new_password = "un-nouveau-mot-de-passe";
    let wrong = if code == "000000" { "111111" } else { "000000" };
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/reset",
        None,
        Some(json!({ "email": "dan@example.com", "code": wrong, "password": new_password })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_code"))
    );
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/reset",
        None,
        Some(json!({ "email": "dan@example.com", "code": code, "password": "password" })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/reset",
        None,
        Some(json!({ "email": "dan@example.com", "code": code, "password": new_password })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let fresh = b["token"].as_str().unwrap().to_owned();
    for old in [&first, &second] {
        let (s, _) = call(&t.app, "GET", "/auth/account", Some(old), None).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
    }
    let (_, acct) = call(&t.app, "GET", "/auth/account", Some(&fresh), None).await;
    assert_eq!(acct["email_verified"], true, "a reset proves the email");
    let (s, _) = call(
        &t.app,
        "POST",
        "/auth/reset",
        None,
        Some(json!({ "email": "dan@example.com", "code": code, "password": new_password })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(
        login(&t, "dan@example.com", PASSWORD).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        login(&t, "dan@example.com", new_password).await.0,
        StatusCode::OK
    );

    let no_mail = app(state(base_config()).await.unwrap());
    let (s, b) = call(
        &no_mail,
        "POST",
        "/auth/forgot",
        None,
        Some(json!({ "email": "dan@example.com" })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Some("email_not_configured")
        )
    );
    let (_, b) = call(
        &no_mail,
        "POST",
        "/auth/signup",
        None,
        Some(json!({ "email": "eve@example.com", "password": PASSWORD, "name": "Eve" })),
    )
    .await;
    assert_eq!(b["email_sent"], false);
}

#[tokio::test]
async fn sessions_list_revoke_and_password_change() {
    let t = setup().await;
    let (a, _) = signup(&t, "fay@example.com", "Fay").await;
    let mut tokens = vec![a];
    for device in ["Web%20%C2%B7%20Safari", "Pixel"] {
        let r = send(
            &t.app,
            "POST",
            "/auth/login",
            None,
            Some(json!({ "email": "fay@example.com", "password": PASSWORD })),
            &[("x-device", device)],
        )
        .await;
        tokens.push(r.body["token"].as_str().unwrap().to_owned());
    }
    let (_, list) = call(&t.app, "GET", "/auth/sessions", Some(&tokens[0]), None).await;
    let list = list.as_array().unwrap().clone();
    assert_eq!(list.len(), 3);
    assert_eq!(list.iter().filter(|s| s["current"] == true).count(), 1);
    assert!(list.iter().any(|s| s["device"] == "Web · Safari"));
    let pixel = list.iter().find(|s| s["device"] == "Pixel").unwrap();
    assert_eq!(pixel["id"].as_str().unwrap().len(), 16);
    let (s, _) = call(
        &t.app,
        "DELETE",
        &format!("/auth/sessions/{}", pixel["id"].as_str().unwrap()),
        Some(&tokens[0]),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = call(&t.app, "GET", "/auth/account", Some(&tokens[2]), None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (s, _) = call(
        &t.app,
        "DELETE",
        "/auth/sessions/0123456789abcdef",
        Some(&tokens[0]),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/password",
        Some(&tokens[0]),
        Some(json!({ "current": "nope-nope-nope", "password": "encore-un-autre-mdp" })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_credentials"))
    );
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/password",
        Some(&tokens[0]),
        Some(json!({ "current": PASSWORD, "password": "encore-un-autre-mdp", "revoke_others": true })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["revoked"], 1);
    let (s, _) = call(&t.app, "GET", "/auth/account", Some(&tokens[1]), None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    assert_eq!(
        login(&t, "fay@example.com", "encore-un-autre-mdp").await.0,
        StatusCode::OK
    );

    let (_, b) = call(
        &t.app,
        "POST",
        "/auth/sessions/revoke-others",
        Some(&tokens[0]),
        None,
    )
    .await;
    assert_eq!(b["revoked"], 1);
    let (s, _) = call(&t.app, "POST", "/auth/logout", Some(&tokens[0]), None).await;
    assert_eq!(s, StatusCode::OK);
    let (s, b) = call(&t.app, "GET", "/auth/account", Some(&tokens[0]), None).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::UNAUTHORIZED, Some("unauthorized"))
    );
}

#[tokio::test]
async fn legacy_accounts_keep_working_and_can_be_secured() {
    let t = setup().await;
    let (s, b) = legacy_join(&t.app, "Louis").await;
    assert_eq!(s, StatusCode::OK);
    let louis = b["token"].as_str().unwrap().to_owned();
    let louis_id = b["member"]["id"].as_i64().unwrap();
    let (s, b) = legacy_join(&t.app, "Marc").await;
    assert_eq!(s, StatusCode::OK);
    let marc = b["token"].as_str().unwrap().to_owned();
    let (s, b) = legacy_join(&t.app, "marc").await;
    assert_eq!(s, StatusCode::OK);
    assert_ne!(b["token"].as_str().unwrap(), marc);

    let (_, list) = call(&t.app, "GET", "/auth/sessions", Some(&louis), None).await;
    assert_eq!(list[0]["device"], Value::Null);
    let r = send(
        &t.app,
        "GET",
        "/me",
        Some(&louis),
        None,
        &[("x-device", "iPhone%20%C2%B7%20iOS%2018")],
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    let (_, list) = call(&t.app, "GET", "/auth/sessions", Some(&louis), None).await;
    assert_eq!(list[0]["device"], "iPhone · iOS 18");

    let (_, acct) = call(&t.app, "GET", "/auth/account", Some(&louis), None).await;
    assert_eq!(acct["secured"], false);
    assert_eq!(acct["email"], Value::Null);
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/credentials",
        Some(&louis),
        Some(json!({ "email": "louis@example.com", "password": PASSWORD })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["email_sent"], true);
    assert!(t.mail.last_code("louis@example.com").is_some());
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/credentials",
        Some(&louis),
        Some(json!({ "email": "other@example.com", "password": PASSWORD })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("already_secured"))
    );
    let (s, me) = call(&t.app, "GET", "/me", Some(&louis), None).await;
    assert_eq!((s, me["id"].as_i64()), (StatusCode::OK, Some(louis_id)));
    let (s, b) = legacy_join(&t.app, "louis").await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("account_protected"))
    );
    let (s, b) = login(&t, "louis@example.com", PASSWORD).await;
    assert_eq!(s, StatusCode::OK);
    let (s, me) = call(&t.app, "GET", "/me", b["token"].as_str(), None).await;
    assert_eq!((s, me["id"].as_i64()), (StatusCode::OK, Some(louis_id)));

    let strict = with_mailer(Config {
        auth_required: true,
        ..base_config()
    })
    .await;
    let (s, b) = legacy_join(&strict.app, "Nina").await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::GONE, Some("sign_in_required"))
    );
    let (s, _) = call(
        &strict.app,
        "POST",
        "/start",
        None,
        Some(json!({ "name": "Nina", "room": "Chez Nina" })),
    )
    .await;
    assert_eq!(s, StatusCode::GONE);
}

#[tokio::test]
async fn features_advertise_only_what_is_configured() {
    let plain = app(state(base_config()).await.unwrap());
    let (_, f) = call(&plain, "GET", "/features", None, None).await;
    let f: Vec<String> = serde_json::from_value(f).unwrap();
    assert!(f.contains(&"auth".into()) && f.contains(&"rooms".into()));
    assert!(!f.iter().any(|x| x == "email" || x.starts_with("oauth_")));
    let (_, p) = call(&plain, "GET", "/auth/providers", None, None).await;
    assert_eq!(p, json!([]));

    let client = OAuthClient {
        id: "id".into(),
        secret: "secret".into(),
        endpoints: None,
    };
    let no_url = with_mailer(Config {
        discord: Some(client.clone()),
        ..base_config()
    })
    .await;
    let (_, f) = call(&no_url.app, "GET", "/features", None, None).await;
    assert!(f.as_array().unwrap().contains(&json!("email")));
    assert!(!f.as_array().unwrap().contains(&json!("oauth_discord")));

    let full = with_mailer(Config {
        public_url: Some(PUBLIC_URL.into()),
        google: Some(client.clone()),
        discord: Some(client),
        ..base_config()
    })
    .await;
    let (_, f) = call(&full.app, "GET", "/features", None, None).await;
    for name in ["auth", "email", "oauth_google", "oauth_discord"] {
        assert!(f.as_array().unwrap().contains(&json!(name)), "{name}");
    }
    let (_, p) = call(&full.app, "GET", "/auth/providers", None, None).await;
    assert_eq!(p, json!(["google", "discord"]));
}
