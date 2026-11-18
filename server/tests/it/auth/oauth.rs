use super::*;

#[tokio::test]
async fn oauth_discord_new_account_auto_link_and_link_mode() {
    let (t, grants) = oauth_setup().await;
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/oauth/discord/start",
        None,
        Some(json!({ "app_redirect": "https://evil.example/oauth" })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_redirect"))
    );
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/oauth/apple/start",
        None,
        Some(json!({ "app_redirect": "quidenous://oauth" })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::NOT_FOUND, Some("unknown_provider"))
    );
    let (s, _) = call(
        &t.app,
        "POST",
        "/auth/oauth/discord/start",
        None,
        Some(json!({ "app_redirect": format!("{PUBLIC_URL}/oauth") })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);

    // New account from a verified Discord email.
    let (location, key) = oauth_round(
        &t,
        &grants,
        "discord",
        None,
        false,
        discord("111", "gege@example.com", true),
    )
    .await;
    // The code alone (hijacked redirect) gets nothing.
    let (s, b) = redeem(&t, &location, "not-the-key").await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_code"))
    );
    let (s, b) = redeem(&t, &location, &key).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["new"], true);
    let gege = b["token"].as_str().unwrap().to_owned();
    let gege_id = b["user_id"].as_i64().unwrap();
    let (s, _) = redeem(&t, &location, &key).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "one time only");
    let (_, acct) = call(&t.app, "GET", "/auth/account", Some(&gege), None).await;
    assert_eq!(acct["name"], "Gégé");
    assert_eq!(acct["email"], "gege@example.com");
    assert_eq!(acct["email_verified"], true);
    assert_eq!(acct["providers"], json!(["discord"]));
    assert_eq!(acct["secured"], true);
    let (s, b) = call(&t.app, "GET", "/me", Some(&gege), None).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("no_room"))
    );
    // Signing in again finds the same account.
    let (location, key) = oauth_round(
        &t,
        &grants,
        "discord",
        None,
        false,
        discord("111", "gege@example.com", true),
    )
    .await;
    let (_, b) = redeem(&t, &location, &key).await;
    assert_eq!(
        (b["user_id"].as_i64(), &b["new"]),
        (Some(gege_id), &json!(false))
    );
    // Unlinking needs a fresh round with the provider first (no password to prove it with).
    let (s, b) = call(&t.app, "DELETE", "/auth/oauth/discord", Some(&gege), None).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("reauth_required"))
    );
    let (s, b) = oauth_reauth(
        &t,
        &grants,
        "discord",
        &gege,
        discord("111", "gege@example.com", true),
    )
    .await;
    assert_eq!((s, &b["reauth"]), (StatusCode::OK, &json!(true)), "{b}");
    // Last way in: can't unlink.
    let (s, b) = call(&t.app, "DELETE", "/auth/oauth/discord", Some(&gege), None).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("last_method"))
    );

    // Auto-link: same verified email on both sides.
    let (hal, hal_id) = signup(&t, "hal@example.com", "Hal").await;
    verify_email(&t, &hal, "hal@example.com").await;
    let (location, key) = oauth_round(
        &t,
        &grants,
        "discord",
        None,
        false,
        discord("222", "hal@example.com", true),
    )
    .await;
    let (_, b) = redeem(&t, &location, &key).await;
    assert_eq!(
        (b["user_id"].as_i64(), &b["new"]),
        (Some(hal_id), &json!(false))
    );
    // Hal has a password too: unlinking is fine, once re-authenticated.
    let (s, _) = call(&t.app, "DELETE", "/auth/oauth/discord", Some(&hal), None).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    reauth(&t, &hal).await;
    let (s, _) = call(&t.app, "DELETE", "/auth/oauth/discord", Some(&hal), None).await;
    assert_eq!(s, StatusCode::OK);

    // Never on an unverified email: neither the account's nor the provider's.
    signup(&t, "ivy@example.com", "Ivy").await;
    let (location, key) = oauth_round(
        &t,
        &grants,
        "discord",
        None,
        false,
        discord("333", "ivy@example.com", true),
    )
    .await;
    let (s, b) = redeem(&t, &location, &key).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("email_taken"))
    );
    let (location, key) = oauth_round(
        &t,
        &grants,
        "discord",
        None,
        false,
        discord("444", "hal@example.com", false),
    )
    .await;
    let (s, b) = redeem(&t, &location, &key).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(b["new"], true);
    assert_ne!(b["user_id"].as_i64(), Some(hal_id));

    // Link mode: a legacy account adds Discord.
    let (_, b) = legacy_join(&t.app, "Jo").await;
    let jo = b["token"].as_str().unwrap().to_owned();
    let (s, _) = call(
        &t.app,
        "POST",
        "/auth/oauth/discord/start",
        None,
        Some(json!({ "app_redirect": "quidenous://oauth", "link": true })),
    )
    .await;
    assert_eq!(s, StatusCode::UNAUTHORIZED, "link mode needs a session");
    let (location, key) = oauth_round(
        &t,
        &grants,
        "discord",
        Some(&jo),
        true,
        discord("555", "jo@example.com", true),
    )
    .await;
    let (s, b) = redeem(&t, &location, &key).await;
    assert_eq!((s, &b), (StatusCode::OK, &json!({ "linked": "discord" })));
    let (_, acct) = call(&t.app, "GET", "/auth/account", Some(&jo), None).await;
    assert_eq!(acct["providers"], json!(["discord"]));
    assert_eq!(acct["secured"], true);
    assert_eq!(
        acct["email"], "jo@example.com",
        "the provider vouches for it"
    );
    let (s, b) = legacy_join(&t.app, "jo").await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("account_protected"))
    );
    // Someone else's identity can't be linked.
    let (location, key) = oauth_round(
        &t,
        &grants,
        "discord",
        Some(&hal),
        true,
        discord("111", "gege@example.com", true),
    )
    .await;
    let (s, b) = redeem(&t, &location, &key).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("identity_taken"))
    );

    // The provider says no: back to the app with an error.
    let (_, b) = call(
        &t.app,
        "POST",
        "/auth/oauth/discord/start",
        None,
        Some(json!({ "app_redirect": "quidenous://oauth" })),
    )
    .await;
    let csrf = query_param(b["url"].as_str().unwrap(), "state").unwrap();
    let r = send(
        &t.app,
        "GET",
        &format!("/auth/oauth/discord/callback?error=access_denied&state={csrf}"),
        None,
        None,
        &[],
    )
    .await;
    assert_eq!(r.headers["location"], "quidenous://oauth?error=cancelled");
}

#[tokio::test]
async fn oauth_google_id_token_is_verified() {
    let (t, grants) = oauth_setup().await;
    let claims = |sub: &'static str, aud: &'static str, good_nonce: bool| {
        move |nonce: &str| {
            json!({
                "iss": "https://accounts.google.com",
                "aud": aud,
                "sub": sub,
                "exp": now() + 600,
                "iat": now(),
                "nonce": if good_nonce { nonce.to_owned() } else { "forged".to_owned() },
                "email": "kim@example.com",
                "email_verified": true,
                "given_name": "Kim",
            })
        }
    };
    let (location, key) = oauth_round(
        &t,
        &grants,
        "google",
        None,
        false,
        claims("g-1", "client-id", true),
    )
    .await;
    let (s, b) = redeem(&t, &location, &key).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["new"], true);
    let (_, acct) = call(&t.app, "GET", "/auth/account", b["token"].as_str(), None).await;
    assert_eq!(
        (&acct["name"], &acct["providers"]),
        (&json!("Kim"), &json!(["google"]))
    );

    // Wrong nonce or audience: refused.
    for bad in [
        claims("g-2", "client-id", false),
        claims("g-3", "someone-else", true),
    ] {
        let (location, _) = oauth_round(&t, &grants, "google", None, false, bad).await;
        assert_eq!(location, "quidenous://oauth?error=provider_error");
    }

    // 2FA still applies to OAuth sign-ins. Turning it on without a password: a fresh Google round.
    let token = b["token"].as_str().unwrap().to_owned();
    let (s, b) = oauth_reauth(
        &t,
        &grants,
        "google",
        &token,
        claims("g-1", "client-id", true),
    )
    .await;
    assert_eq!((s, &b["reauth"]), (StatusCode::OK, &json!(true)), "{b}");
    let (_, setup) = call(&t.app, "POST", "/auth/2fa/setup", Some(&token), None).await;
    let secret = setup["secret"].as_str().unwrap().to_owned();
    let (s, _) = call(
        &t.app,
        "POST",
        "/auth/2fa/enable",
        Some(&token),
        Some(json!({ "code": totp_code(&secret, now()).unwrap() })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (location, key) = oauth_round(
        &t,
        &grants,
        "google",
        None,
        false,
        claims("g-1", "client-id", true),
    )
    .await;
    let (_, b) = redeem(&t, &location, &key).await;
    assert_eq!(b["mfa"], "totp");
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/login/2fa",
        None,
        Some(
            json!({ "challenge": b["challenge"], "code": totp_code(&secret, now() + 30).unwrap() }),
        ),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
}

#[tokio::test]
async fn parallel_guesses_share_the_same_budgets() {
    let t = setup().await;
    let (token, _) = signup(&t, "gus@example.com", "Gus").await;
    let code = t.mail.last_code("gus@example.com").unwrap();
    let wrong = if code == "000000" { "111111" } else { "000000" };
    // 12 guesses at once: a code still only gets 5.
    let guesses = (0..12).map(|_| {
        call(
            &t.app,
            "POST",
            "/auth/verify",
            Some(&token),
            Some(json!({ "code": wrong })),
        )
    });
    let results = futures::future::join_all(guesses).await;
    let errors = |code: &str| results.iter().filter(|(_, b)| b["error"] == code).count();
    assert_eq!((errors("bad_code"), errors("expired")), (5, 7));
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/verify",
        Some(&token),
        Some(json!({ "code": code })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("expired"))
    );

    // Sign-in: successes give their token back, parallel failures each take one.
    for _ in 0..6 {
        assert_eq!(
            login(&t, "gus@example.com", PASSWORD).await.0,
            StatusCode::OK
        );
    }
    let tries = (0..8).map(|_| login(&t, "gus@example.com", "not-the-password"));
    let results = futures::future::join_all(tries).await;
    let status = |s: StatusCode| results.iter().filter(|(r, _)| *r == s).count();
    assert_eq!(
        (
            status(StatusCode::BAD_REQUEST),
            status(StatusCode::TOO_MANY_REQUESTS)
        ),
        (5, 3)
    );
    assert_eq!(
        login(&t, "gus@example.com", PASSWORD).await.0,
        StatusCode::TOO_MANY_REQUESTS
    );

    // Reset: 10 tries per hour per email, whatever the IP.
    for i in 0..10 {
        let ip = format!("192.0.2.{i}");
        let r = send(
            &t.app,
            "POST",
            "/auth/reset",
            None,
            Some(json!({ "email": "gus@example.com", "code": "123456", "password": "un-nouveau-mot-de-passe" })),
            &[("x-forwarded-for", &ip)],
        )
        .await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST);
    }
    let r = send(
        &t.app,
        "POST",
        "/auth/reset",
        None,
        Some(json!({ "email": "gus@example.com", "code": "123456", "password": "un-nouveau-mot-de-passe" })),
        &[("x-forwarded-for", "192.0.2.99")],
    )
    .await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn a_name_grab_cannot_lock_the_first_device_out() {
    let t = setup().await;
    let (_, b) = legacy_join(&t.app, "Rita").await;
    let rita = b["token"].as_str().unwrap().to_owned();
    let (s, b) = legacy_join(&t.app, "rita").await;
    assert_eq!(s, StatusCode::OK);
    let grab = b["token"].as_str().unwrap().to_owned();
    let rita_session = session_id(&t, &rita).await;
    for (method, path, body) in [
        (
            "POST",
            "/auth/credentials".to_owned(),
            Some(json!({ "email": "grab@example.com", "password": PASSWORD })),
        ),
        (
            "POST",
            "/auth/password".into(),
            Some(json!({ "password": PASSWORD })),
        ),
        ("POST", "/auth/2fa/setup".into(), None),
        (
            "POST",
            "/auth/2fa/enable".into(),
            Some(json!({ "code": "123456" })),
        ),
        ("POST", "/auth/sessions/revoke-others".into(), None),
        ("DELETE", format!("/auth/sessions/{rita_session}"), None),
    ] {
        let (s, b) = call(&t.app, method, &path, Some(&grab), body).await;
        assert_eq!(
            (s, b["error"].as_str()),
            (StatusCode::FORBIDDEN, Some("claimed_session")),
            "{path}"
        );
    }
    // It still plays in the room.
    assert_eq!(
        call(&t.app, "GET", "/me", Some(&grab), None).await.0,
        StatusCode::OK
    );
    // The first device (it created the account) can sign the claimer out.
    let (_, b) = call(
        &t.app,
        "POST",
        "/auth/sessions/revoke-others",
        Some(&rita),
        None,
    )
    .await;
    assert_eq!(b["revoked"], 1);
    assert_eq!(
        call(&t.app, "GET", "/auth/account", Some(&grab), None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );

    // Alone on the account, a claimed session may secure it: nothing is taken from anyone.
    let (_, b) = legacy_join(&t.app, "RITA").await;
    let grab = b["token"].as_str().unwrap().to_owned();
    call(&t.app, "POST", "/auth/logout", Some(&rita), None).await;
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/credentials",
        Some(&grab),
        Some(json!({ "email": "rita@example.com", "password": PASSWORD })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    // A password sign-in is never claimed; the claimed one is limited again, but can sign itself out.
    let (_, b) = login(&t, "rita@example.com", PASSWORD).await;
    let fresh = b["token"].as_str().unwrap().to_owned();
    let (s, _) = call(
        &t.app,
        "POST",
        "/auth/sessions/revoke-others",
        Some(&grab),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let own = session_id(&t, &grab).await;
    let (s, _) = call(
        &t.app,
        "DELETE",
        &format!("/auth/sessions/{own}"),
        Some(&grab),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, b) = call(
        &t.app,
        "POST",
        "/auth/sessions/revoke-others",
        Some(&fresh),
        None,
    )
    .await;
    assert_eq!(b["revoked"], 0);
}

#[tokio::test]
async fn a_reset_takes_back_an_email_someone_else_typed() {
    let (t, grants) = oauth_setup().await;
    // Someone signs up with Vic's address (never verified), links their own Discord, turns 2FA on.
    let (squatter, squatter_id) = signup(&t, "vic@example.com", "Vic").await;
    reauth(&t, &squatter).await;
    let (location, key) = oauth_round(
        &t,
        &grants,
        "discord",
        Some(&squatter),
        true,
        discord("666", "squatter@example.com", true),
    )
    .await;
    assert_eq!(
        redeem(&t, &location, &key).await.1,
        json!({ "linked": "discord" })
    );
    let (_, setup) = call(&t.app, "POST", "/auth/2fa/setup", Some(&squatter), None).await;
    let secret = setup["secret"].as_str().unwrap().to_owned();
    let (s, _) = call(
        &t.app,
        "POST",
        "/auth/2fa/enable",
        Some(&squatter),
        Some(json!({ "code": totp_code(&secret, now()).unwrap() })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);

    // Vic resets the password: the code proves the address, the account is Vic's alone.
    let forgot = || {
        call(
            &t.app,
            "POST",
            "/auth/forgot",
            None,
            Some(json!({ "email": "vic@example.com" })),
        )
    };
    forgot().await;
    let code = wait_for_code(&t.mail, "vic@example.com", 2).await;
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/reset",
        None,
        Some(
            json!({ "email": "vic@example.com", "code": code, "password": "le-vrai-mot-de-passe" }),
        ),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["user_id"].as_i64(), Some(squatter_id));
    let vic = b["token"]
        .as_str()
        .expect("no 2FA step: someone else set it up")
        .to_owned();
    let (_, acct) = call(&t.app, "GET", "/auth/account", Some(&vic), None).await;
    assert_eq!(
        (
            &acct["providers"],
            &acct["totp"],
            &acct["recovery_codes_left"],
            &acct["email_verified"]
        ),
        (&json!([]), &json!(false), &json!(0), &json!(true))
    );
    assert_eq!(
        call(&t.app, "GET", "/auth/account", Some(&squatter), None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    // The squatter's Discord doesn't lead there any more.
    let (location, key) = oauth_round(
        &t,
        &grants,
        "discord",
        None,
        false,
        discord("666", "squatter@example.com", true),
    )
    .await;
    let (_, b) = redeem(&t, &location, &key).await;
    assert_eq!(b["new"], true);
    assert_ne!(b["user_id"].as_i64(), Some(squatter_id));

    // Once the address is proven, 2FA set up by its owner still applies to a reset.
    let (s, _) = call(
        &t.app,
        "POST",
        "/auth/reauth",
        Some(&vic),
        Some(json!({ "password": "le-vrai-mot-de-passe" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, setup) = call(&t.app, "POST", "/auth/2fa/setup", Some(&vic), None).await;
    let secret = setup["secret"].as_str().unwrap().to_owned();
    let (s, _) = call(
        &t.app,
        "POST",
        "/auth/2fa/enable",
        Some(&vic),
        Some(json!({ "code": totp_code(&secret, now()).unwrap() })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    forgot().await;
    let code = wait_for_code(&t.mail, "vic@example.com", 3).await;
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/reset",
        None,
        Some(
            json!({ "email": "vic@example.com", "code": code, "password": "encore-un-autre-mdp" }),
        ),
    )
    .await;
    assert_eq!((s, &b["mfa"]), (StatusCode::OK, &json!("totp")), "{b}");
}

#[tokio::test]
async fn oauth_redirect_code_alone_links_and_creates_nothing() {
    let (t, grants) = oauth_setup().await;
    let (_, b) = legacy_join(&t.app, "Ola").await;
    let ola = b["token"].as_str().unwrap().to_owned();
    // Ola starts a link, then gets someone else's browser through the provider: the redirect code
    // without the key from `start` must not attach that person's identity to Ola's account.
    let (location, key) = oauth_round(
        &t,
        &grants,
        "discord",
        Some(&ola),
        true,
        discord("777", "victim@example.com", true),
    )
    .await;
    let users = count(&t, "SELECT COUNT(*) FROM users").await;
    let (s, b) = redeem(&t, &location, "not-the-key").await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_code"))
    );
    let (_, acct) = call(&t.app, "GET", "/auth/account", Some(&ola), None).await;
    assert_eq!(
        (&acct["providers"], &acct["email"]),
        (&json!([]), &Value::Null)
    );
    // Sign-in mode: no account is created either.
    let (other, _) = oauth_round(
        &t,
        &grants,
        "discord",
        None,
        false,
        discord("888", "new@example.com", true),
    )
    .await;
    let (s, _) = redeem(&t, &other, "not-the-key").await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(count(&t, "SELECT COUNT(*) FROM users").await, users);
    assert_eq!(count(&t, "SELECT COUNT(*) FROM oauth_identities").await, 0);
    // With the key, the link goes through.
    let (s, b) = redeem(&t, &location, &key).await;
    assert_eq!((s, b), (StatusCode::OK, json!({ "linked": "discord" })));

    // Link mode from a claimed session (first-name grab): refused while another device exists.
    let (_, b) = legacy_join(&t.app, "Kat").await;
    let kat = b["token"].as_str().unwrap().to_owned();
    let (_, b) = legacy_join(&t.app, "kat").await;
    let grab = b["token"].as_str().unwrap().to_owned();
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/oauth/discord/start",
        Some(&grab),
        Some(json!({ "app_redirect": "quidenous://oauth", "link": true })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("claimed_session"))
    );
    // The first device signs out: the claimer is alone and may start a link...
    call(&t.app, "POST", "/auth/logout", Some(&kat), None).await;
    let (location, key) = oauth_round(
        &t,
        &grants,
        "discord",
        Some(&grab),
        true,
        discord("999", "kat@example.com", true),
    )
    .await;
    // ...but redeem checks again: someone else claimed the name meanwhile.
    legacy_join(&t.app, "KAT").await;
    let (s, b) = redeem(&t, &location, &key).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("claimed_session"))
    );
    let (_, acct) = call(&t.app, "GET", "/auth/account", Some(&grab), None).await;
    assert_eq!(acct["providers"], json!([]));
}
