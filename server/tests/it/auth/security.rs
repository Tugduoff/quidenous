use super::*;

#[tokio::test]
async fn sensitive_changes_need_a_recent_reauth() {
    let (t, grants) = oauth_setup().await;
    let (ana, _) = signup(&t, "ana@example.com", "Ana").await;
    verify_email(&t, &ana, "ana@example.com").await;
    // Without re-authenticating: 2FA set-up and enable, linking, deleting.
    assert_eq!(
        status_error(&t, "POST", "/auth/2fa/setup", &ana, None).await,
        reauth_required()
    );
    assert_eq!(
        status_error(
            &t,
            "POST",
            "/auth/2fa/enable",
            &ana,
            Some(json!({ "code": "123456" }))
        )
        .await,
        reauth_required()
    );
    assert_eq!(
        status_error(
            &t,
            "POST",
            "/auth/oauth/google/start",
            &ana,
            Some(json!({ "app_redirect": "quidenous://oauth", "link": true }))
        )
        .await,
        reauth_required()
    );
    assert_eq!(
        status_error(&t, "DELETE", "/auth/account", &ana, None).await,
        reauth_required()
    );

    // Wrong or no proof.
    assert_eq!(
        status_error(
            &t,
            "POST",
            "/auth/reauth",
            &ana,
            Some(json!({ "password": "pas-le-bon-mot" }))
        )
        .await,
        (StatusCode::BAD_REQUEST, Some("bad_credentials".into()))
    );
    assert_eq!(
        status_error(
            &t,
            "POST",
            "/auth/reauth",
            &ana,
            Some(json!({ "code": "123456" }))
        )
        .await,
        (StatusCode::BAD_REQUEST, Some("bad_credentials".into())),
        "no 2FA: a code proves nothing"
    );
    let (s, _) = call(
        &t.app,
        "POST",
        "/auth/reauth",
        None,
        Some(json!({ "password": PASSWORD })),
    )
    .await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);

    // The right password: this session only, for 10 minutes.
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/reauth",
        Some(&ana),
        Some(json!({ "password": PASSWORD })),
    )
    .await;
    assert_eq!((s, &b["expires_in"]), (StatusCode::OK, &json!(600)), "{b}");
    let (_, other) = login(&t, "ana@example.com", PASSWORD).await;
    let other = other["token"].as_str().unwrap().to_owned();
    assert_eq!(
        status_error(&t, "POST", "/auth/2fa/setup", &other, None).await,
        reauth_required()
    );
    let (s, setup) = call(&t.app, "POST", "/auth/2fa/setup", Some(&ana), None).await;
    assert_eq!(s, StatusCode::OK, "{setup}");
    let secret = setup["secret"].as_str().unwrap().to_owned();
    // Linking works too now.
    let (location, key) = oauth_round(
        &t,
        &grants,
        "discord",
        Some(&ana),
        true,
        discord("a-1", "ana@example.com", true),
    )
    .await;
    assert_eq!(
        redeem(&t, &location, &key).await.1,
        json!({ "linked": "discord" })
    );

    // Ten minutes later it has to be done again.
    sqlx::query("UPDATE user_tokens SET reauth_at = reauth_at - 601 WHERE reauth_at IS NOT NULL")
        .execute(&t.state.db)
        .await
        .unwrap();
    let code = totp_code(&secret, now()).unwrap();
    assert_eq!(
        status_error(
            &t,
            "POST",
            "/auth/2fa/enable",
            &ana,
            Some(json!({ "code": code }))
        )
        .await,
        reauth_required()
    );
    assert_eq!(
        status_error(&t, "DELETE", "/auth/oauth/discord", &ana, None).await,
        reauth_required()
    );
    reauth(&t, &ana).await;
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/2fa/enable",
        Some(&ana),
        Some(json!({ "code": code })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let recovery = b["recovery_codes"][0].as_str().unwrap().to_owned();

    // With 2FA on, a code from the app or a recovery code proves it too (from another session).
    let (s, b) = login(&t, "ana@example.com", PASSWORD).await;
    assert_eq!(s, StatusCode::OK);
    let (_, b) = call(
        &t.app,
        "POST",
        "/auth/login/2fa",
        None,
        Some(json!({ "challenge": b["challenge"], "code": recovery })),
    )
    .await;
    let third = b["token"].as_str().unwrap().to_owned();
    assert_eq!(
        status_error(&t, "DELETE", "/auth/oauth/discord", &third, None).await,
        reauth_required()
    );
    let (s, _) = call(
        &t.app,
        "POST",
        "/auth/reauth",
        Some(&third),
        Some(json!({ "code": recovery })),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "a recovery code is single use");
    let other_recovery = {
        // A fresh set: the old ones go.
        let code = totp_code(&secret, now() + 30).unwrap();
        let (s, b) = call(
            &t.app,
            "POST",
            "/auth/2fa/recovery",
            Some(&third),
            Some(json!({ "code": code })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{b}");
        b["recovery_codes"][0].as_str().unwrap().to_owned()
    };
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/reauth",
        Some(&third),
        Some(json!({ "code": other_recovery })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let (s, _) = call(&t.app, "DELETE", "/auth/oauth/discord", Some(&third), None).await;
    assert_eq!(s, StatusCode::OK);

    // Deleting: the password inline still works (older apps), as does a recent re-auth.
    let (s, _) = call(&t.app, "DELETE", "/auth/account", Some(&third), None).await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = call(&t.app, "GET", "/auth/account", Some(&ana), None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn provider_only_accounts_reauth_with_a_fresh_provider_round() {
    let (t, grants) = oauth_setup().await;
    let (location, key) = oauth_round(
        &t,
        &grants,
        "discord",
        None,
        false,
        discord("p-1", "pia@example.com", true),
    )
    .await;
    let (_, b) = redeem(&t, &location, &key).await;
    let pia = b["token"].as_str().unwrap().to_owned();
    // A first password, linking another provider, 2FA, deleting: all need it.
    assert_eq!(
        status_error(
            &t,
            "POST",
            "/auth/password",
            &pia,
            Some(json!({ "password": PASSWORD }))
        )
        .await,
        reauth_required()
    );
    assert_eq!(
        status_error(&t, "POST", "/auth/2fa/setup", &pia, None).await,
        reauth_required()
    );
    assert_eq!(
        status_error(&t, "DELETE", "/auth/account", &pia, None).await,
        reauth_required()
    );
    // No password to type: a password proves nothing.
    assert_eq!(
        status_error(
            &t,
            "POST",
            "/auth/reauth",
            &pia,
            Some(json!({ "password": PASSWORD }))
        )
        .await,
        (StatusCode::BAD_REQUEST, Some("bad_credentials".into()))
    );
    // Re-auth mode needs a session.
    let (s, _) = call(
        &t.app,
        "POST",
        "/auth/oauth/discord/start",
        None,
        Some(json!({ "app_redirect": "quidenous://oauth", "reauth": true })),
    )
    .await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    // Another Discord account than the linked one proves nothing, and links nothing.
    let (s, b) = oauth_reauth(
        &t,
        &grants,
        "discord",
        &pia,
        discord("p-2", "other@example.com", true),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("wrong_account"))
    );
    assert_eq!(
        count(
            &t,
            "SELECT COUNT(*) FROM oauth_identities WHERE subject = 'p-2'"
        )
        .await,
        0
    );
    assert_eq!(
        count(&t, "SELECT COUNT(*) FROM users").await,
        1,
        "no account created either"
    );
    assert_eq!(
        status_error(
            &t,
            "POST",
            "/auth/password",
            &pia,
            Some(json!({ "password": PASSWORD }))
        )
        .await,
        reauth_required()
    );
    // The linked one does.
    let (s, b) = oauth_reauth(
        &t,
        &grants,
        "discord",
        &pia,
        discord("p-1", "pia@example.com", true),
    )
    .await;
    assert_eq!((s, &b), (StatusCode::OK, &json!({ "reauth": true })));
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/password",
        Some(&pia),
        Some(json!({ "password": PASSWORD })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");

    // A Discord account that gave no e-mail adds one with a password: same rule.
    let (location, key) = oauth_round(
        &t,
        &grants,
        "discord",
        None,
        false,
        discord("p-3", "", false),
    )
    .await;
    let (_, b) = redeem(&t, &location, &key).await;
    let noe = b["token"].as_str().unwrap().to_owned();
    let creds = json!({ "email": "noe@example.com", "password": PASSWORD });
    assert_eq!(
        status_error(&t, "POST", "/auth/credentials", &noe, Some(creds.clone())).await,
        reauth_required()
    );
    let (s, _) = oauth_reauth(&t, &grants, "discord", &noe, discord("p-3", "", false)).await;
    assert_eq!(s, StatusCode::OK);
    let (s, b) = call(&t.app, "POST", "/auth/credentials", Some(&noe), Some(creds)).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    // The re-auth also lets it delete the account.
    let (s, _) = call(&t.app, "DELETE", "/auth/account", Some(&noe), None).await;
    assert_eq!(s, StatusCode::OK);
    // A signed-out session can't finish a re-auth round.
    let (location, key) = oauth_round_with(
        &t,
        &grants,
        "discord",
        Some(&pia),
        json!({ "reauth": true }),
        discord("p-1", "pia@example.com", true),
    )
    .await;
    call(&t.app, "POST", "/auth/logout", Some(&pia), None).await;
    let (s, _) = redeem(&t, &location, &key).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn legacy_accounts_have_nothing_to_reauth_with() {
    let t = setup().await;
    let (_, b) = legacy_join(&t.app, "Léo").await;
    let leo = b["token"].as_str().unwrap().to_owned();
    let (s, b) = call(&t.app, "POST", "/auth/reauth", Some(&leo), Some(json!({}))).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_credentials"))
    );
    // Securing the account and deleting it need no re-auth: there is nothing to prove with.
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/credentials",
        Some(&leo),
        Some(json!({ "email": "leo@example.com", "password": PASSWORD })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    // Now it has a password: the next sensitive change needs it.
    assert_eq!(
        status_error(&t, "POST", "/auth/2fa/setup", &leo, None).await,
        reauth_required()
    );
}

#[tokio::test]
async fn wrong_codes_are_capped_per_email_across_restarts() {
    let config = base_config();
    let t = with_mailer(config.clone()).await;
    signup(&t, "eve@example.com", "Eve").await;
    let reset = |code: &str| json!({ "email": "eve@example.com", "code": code, "password": "un-autre-mot-de-passe" });
    // A right code under the cap works and isn't counted.
    let code = forgot_code(&t, "eve@example.com").await;
    let (s, _) = call(
        &t.app,
        "POST",
        "/auth/reset",
        None,
        Some(reset(wrong_code(&code))),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, b) = call(&t.app, "POST", "/auth/reset", None, Some(reset(&code))).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(count(&t, "SELECT COUNT(*) FROM code_failures").await, 1);

    // 19 more wrong codes, over several "restarts" (fresh in-memory limits each time), reset and
    // delete codes alike: 5 tries per code, then a new code.
    let mut failures = 1;
    while failures < 20 {
        let t = with_mailer(config.clone()).await;
        let delete = failures % 2 == 1;
        let code = if delete {
            let before = t.mail.count("eve@example.com");
            call(
                &t.app,
                "POST",
                "/auth/delete/send",
                None,
                Some(json!({ "email": "eve@example.com" })),
            )
            .await;
            wait_for_code(&t.mail, "eve@example.com", before + 1).await
        } else {
            forgot_code(&t, "eve@example.com").await
        };
        for _ in 0..5 {
            if failures == 20 {
                break;
            }
            let (s, b) = if delete {
                call(
                    &t.app,
                    "POST",
                    "/auth/delete/confirm",
                    None,
                    Some(json!({ "email": "eve@example.com", "code": wrong_code(&code) })),
                )
                .await
            } else {
                call(
                    &t.app,
                    "POST",
                    "/auth/reset",
                    None,
                    Some(reset(wrong_code(&code))),
                )
                .await
            };
            assert_eq!(
                (s, b["error"].as_str()),
                (StatusCode::BAD_REQUEST, Some("bad_code")),
                "failure {failures}"
            );
            failures += 1;
        }
    }
    // The 20th failure killed the code; after a restart, a new code is refused even when right,
    // for every kind, until a day has passed.
    let t = with_mailer(config.clone()).await;
    let code = forgot_code(&t, "eve@example.com").await;
    let r = send(&t.app, "POST", "/auth/reset", None, Some(reset(&code)), &[]).await;
    assert_eq!(
        (r.status, r.body["error"].as_str()),
        (StatusCode::TOO_MANY_REQUESTS, Some("rate_limited"))
    );
    let wait = r.body["retry_after"].as_u64().unwrap();
    assert!((80_000..=86_400).contains(&wait), "{wait}");
    // That code is dead now, and refused tries don't count.
    let (s, b) = call(&t.app, "POST", "/auth/reset", None, Some(reset(&code))).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_code"))
    );
    assert_eq!(count(&t, "SELECT COUNT(*) FROM code_failures").await, 20);
    let before = t.mail.count("eve@example.com");
    call(
        &t.app,
        "POST",
        "/auth/delete/send",
        None,
        Some(json!({ "email": "eve@example.com" })),
    )
    .await;
    let code = wait_for_code(&t.mail, "eve@example.com", before + 1).await;
    let (s, _) = call(
        &t.app,
        "POST",
        "/auth/delete/confirm",
        None,
        Some(json!({ "email": "eve@example.com", "code": code })),
    )
    .await;
    assert_eq!(s, StatusCode::TOO_MANY_REQUESTS);
    // Another e-mail is not affected.
    signup(&t, "fay@example.com", "Fay").await;
    let code = forgot_code(&t, "fay@example.com").await;
    let (s, _) = call(
        &t.app,
        "POST",
        "/auth/reset",
        None,
        Some(json!({ "email": "fay@example.com", "code": code, "password": "un-autre-mot-de-passe" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);

    // A day later (the oldest failures age out), codes work again.
    sqlx::query("UPDATE code_failures SET at = at - 86400")
        .execute(&t.state.db)
        .await
        .unwrap();
    let code = forgot_code(&t, "eve@example.com").await;
    let (s, b) = call(&t.app, "POST", "/auth/reset", None, Some(reset(&code))).await;
    assert_eq!(s, StatusCode::OK, "{b}");
}

#[tokio::test]
async fn a_lockout_from_one_ip_does_not_lock_the_owner_out() {
    let t = setup().await;
    signup(&t, "gus@example.com", "Gus").await;
    let attempt = |password: &'static str, ip: String| {
        let app = t.app.clone();
        async move {
            send(
                &app,
                "POST",
                "/auth/login",
                None,
                Some(json!({ "email": "gus@example.com", "password": password })),
                &[("x-forwarded-for", &ip)],
            )
            .await
            .status
        }
    };
    for _ in 0..5 {
        assert_eq!(
            attempt("pas-le-bon", "198.51.100.1".into()).await,
            StatusCode::BAD_REQUEST
        );
    }
    // That IP is locked out of this account, even with the right password…
    assert_eq!(
        attempt(PASSWORD, "198.51.100.1".into()).await,
        StatusCode::TOO_MANY_REQUESTS
    );
    // …but the owner, elsewhere, signs in.
    assert_eq!(
        attempt(PASSWORD, "203.0.113.50".into()).await,
        StatusCode::OK
    );
    // Guesses spread over many IPs still stop: 40 failures an hour per e-mail.
    let mut statuses = Vec::new();
    for i in 0..36 {
        statuses.push(attempt("pas-le-bon", format!("192.0.2.{i}")).await);
    }
    assert!(
        statuses[..35].iter().all(|s| *s == StatusCode::BAD_REQUEST),
        "{statuses:?}"
    );
    assert_eq!(statuses[35], StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        attempt(PASSWORD, "203.0.113.51".into()).await,
        StatusCode::TOO_MANY_REQUESTS
    );
}

#[tokio::test]
async fn forwarded_for_uses_the_last_header_line() {
    let t = setup().await;
    // A proxy that adds its own line: the client's line (first) must not pick the IP.
    for i in 0..quidenous_server::accounts::limits::LOGIN_IP.capacity {
        let spoofed = format!("198.18.1.{i}");
        let r = send(
            &t.app,
            "POST",
            "/auth/login",
            None,
            Some(json!({ "email": format!("y{i}@example.com"), "password": "whatever-123" })),
            &[
                ("x-forwarded-for", &spoofed),
                ("x-forwarded-for", "203.0.113.9"),
            ],
        )
        .await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST);
    }
    let r = send(
        &t.app,
        "POST",
        "/auth/login",
        None,
        Some(json!({ "email": "y@example.com", "password": "whatever-123" })),
        &[
            ("x-forwarded-for", "198.18.1.99"),
            ("x-forwarded-for", "203.0.113.9"),
        ],
    )
    .await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn oauth_redirects_are_compared_by_host_not_prefix() {
    let (base, _grants) = mock_provider().await;
    let client = OAuthClient {
        id: "client-id".into(),
        secret: "shh".into(),
        endpoints: Some(OAuthEndpoints {
            authorize: format!("{base}/authorize"),
            token: format!("{base}/token"),
            userinfo: format!("{base}/me"),
            jwks: format!("{base}/jwks"),
        }),
    };
    let t = with_mailer(Config {
        public_url: Some(PUBLIC_URL.into()),
        discord: Some(client),
        oauth_redirects: vec!["http://localhost:8081".into(), "exp://".into()],
        ..base_config()
    })
    .await;
    for (redirect, ok) in [
        ("http://localhost:8081/oauth", true),
        ("exp://192.168.1.20:8081/--/oauth", true),
        ("http://localhost:8081.evil.com/oauth", false),
        ("exp://evil.com/--/oauth", false),
        ("https://qdn.test.evil.com/oauth", false),
    ] {
        let (s, b) = call(
            &t.app,
            "POST",
            "/auth/oauth/discord/start",
            None,
            Some(json!({ "app_redirect": redirect })),
        )
        .await;
        if ok {
            assert_eq!(s, StatusCode::OK, "{redirect}: {b}");
        } else {
            assert_eq!(
                (s, b["error"].as_str()),
                (StatusCode::BAD_REQUEST, Some("bad_redirect")),
                "{redirect}"
            );
        }
    }
}

#[tokio::test]
async fn tickets_replace_the_token_in_urls() {
    let t = setup().await;
    let (ana, _) = signup(&t, "ana@example.com", "Ana").await;
    let room = room_with(&t, &ana, &[]).await;
    let ticket = |purpose: &'static str| {
        let app = t.app.clone();
        let ana = ana.clone();
        async move {
            let (s, b) = call(
                &app,
                "POST",
                "/auth/ticket",
                Some(&ana),
                Some(json!({ "purpose": purpose })),
            )
            .await;
            assert_eq!(s, StatusCode::OK, "{b}");
            assert_eq!(b["expires_in"], 60);
            b["ticket"].as_str().unwrap().to_owned()
        }
    };
    let export = |q: String| {
        let app = t.app.clone();
        async move {
            send(
                &app,
                "GET",
                &format!("/history/export?{q}&room={room}"),
                None,
                None,
                &[],
            )
            .await
            .status
        }
    };
    let (s, _) = call(
        &t.app,
        "POST",
        "/auth/ticket",
        None,
        Some(json!({ "purpose": "export" })),
    )
    .await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let tk = ticket("export").await;
    assert_ne!(tk, ana);
    assert_eq!(export(format!("ticket={tk}")).await, StatusCode::OK);
    assert_eq!(
        export(format!("ticket={tk}")).await,
        StatusCode::UNAUTHORIZED,
        "single use"
    );
    // One purpose: a socket ticket doesn't download the history.
    let tk = ticket("live").await;
    assert_eq!(
        export(format!("ticket={tk}")).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(export("ticket=nope".into()).await, StatusCode::UNAUTHORIZED);
    // It stops working with its session.
    let tk = ticket("export").await;
    // Older apps still send the token itself.
    assert_eq!(export(format!("token={ana}")).await, StatusCode::OK);
    call(&t.app, "POST", "/auth/logout", Some(&ana), None).await;
    assert_eq!(
        export(format!("ticket={tk}")).await,
        StatusCode::UNAUTHORIZED
    );
    // The socket takes tickets too (not a WebSocket request here: refused before the ticket is read).
    let (s, _) = call(&t.app, "GET", "/live?ticket=x", None, None).await;
    assert_ne!(s, StatusCode::OK);
}
