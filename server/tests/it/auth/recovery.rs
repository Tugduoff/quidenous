use super::*;

#[tokio::test]
async fn security_changes_send_a_notice_to_the_verified_email() {
    let t = setup().await;
    // Unverified: the address may be someone else's, nothing is sent to it.
    let (bob, _) = signup(&t, "bob@example.com", "Bob").await;
    let (s, _) = call(
        &t.app,
        "POST",
        "/auth/password",
        Some(&bob),
        Some(json!({ "current": PASSWORD, "password": "un-autre-velo-rouge" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);

    let (ana, _) = signup(&t, "ana@example.com", "Ana").await;
    verify_email(&t, &ana, "ana@example.com").await;
    let secret = enable_2fa(&t, &ana).await;
    assert_eq!(
        wait_notices(&t.mail, "ana@example.com", 1).await,
        ["Double authentification activée"]
    );
    let (s, _) = call(
        &t.app,
        "POST",
        "/auth/password",
        Some(&ana),
        Some(json!({ "current": PASSWORD, "password": "un-autre-velo-rouge" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    wait_notices(&t.mail, "ana@example.com", 2).await;
    tokio::time::sleep(Duration::from_millis(30)).await;
    let code = totp_code(&secret, now() + 30).unwrap();
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/2fa/disable",
        Some(&ana),
        Some(json!({ "code": code })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let notices = wait_notices(&t.mail, "ana@example.com", 3).await;
    assert_eq!(
        notices,
        [
            "Double authentification activée",
            "Mot de passe modifié",
            "Double authentification désactivée"
        ]
    );
    // The notice has no code in it (the code e-mails are unaffected).
    let n = t.mail.notices.lock().unwrap()[0].clone();
    assert!(n.text.contains("Si ce n'est pas toi"), "{}", n.text);
    assert!(n.html.contains("lang=\"fr\""));
    assert!(t.mail.notices("bob@example.com").is_empty());
}

#[tokio::test]
async fn failed_sends_are_reported_without_saying_who_has_an_account() {
    use std::sync::atomic::Ordering;
    let t = setup().await;
    let (ana, _) = signup(&t, "ana@example.com", "Ana").await;
    t.mail.fail.store(true, Ordering::Relaxed);
    // Sign-up: the account works, and the app can say the e-mail didn't go.
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/signup",
        None,
        Some(json!({ "email": "bob@example.com", "password": PASSWORD, "name": "Bob" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(
        (&b["email_sent"], &b["email_failed"]),
        (&json!(false), &json!(true))
    );
    let (s, b) = call(&t.app, "POST", "/auth/verify/send", Some(&ana), None).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::SERVICE_UNAVAILABLE, Some("email_failed"))
    );
    // Forgot and the public deletion page: same answer for an account and for nobody.
    for email in ["ana@example.com", "nobody@example.com"] {
        for path in ["/auth/forgot", "/auth/delete/send"] {
            let (s, b) = call(&t.app, "POST", path, None, Some(json!({ "email": email }))).await;
            assert_eq!(
                (s, b["error"].as_str()),
                (StatusCode::SERVICE_UNAVAILABLE, Some("email_failed")),
                "{path} {email}"
            );
        }
    }
    // Mail works again: the next send clears it.
    t.mail.fail.store(false, Ordering::Relaxed);
    let (s, b) = call(&t.app, "POST", "/auth/verify/send", Some(&ana), None).await;
    assert_eq!((s, &b["email_sent"]), (StatusCode::OK, &json!(true)));
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/forgot",
        None,
        Some(json!({ "email": "nobody@example.com" })),
    )
    .await;
    assert_eq!((s, &b), (StatusCode::OK, &json!({ "sent": true })));
}

#[tokio::test]
async fn changing_the_email_needs_a_code_from_the_new_address() {
    let t = setup().await;
    let (ana, _) = signup(&t, "ana@example.com", "Ana").await;
    verify_email(&t, &ana, "ana@example.com").await;
    signup(&t, "taken@example.com", "Taken").await;
    let change = |email: &'static str| {
        let app = t.app.clone();
        let ana = ana.clone();
        async move {
            call(
                &app,
                "POST",
                "/auth/email/change",
                Some(&ana),
                Some(json!({ "email": email })),
            )
            .await
        }
    };
    assert_eq!(
        change("new@example.com").await.1["error"],
        "reauth_required"
    );
    reauth(&t, &ana).await;
    assert_eq!(change("ana@example.com").await.1["error"], "same_email");
    let (s, b) = change("taken@example.com").await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::CONFLICT, Some("email_taken"))
    );
    let (s, b) = change("New@Example.com").await;
    assert_eq!((s, &b), (StatusCode::OK, &json!({ "sent": true })));
    let code = t.mail.last_code("new@example.com").unwrap();
    let wrong = if code == "000000" { "111111" } else { "000000" };
    let confirm = |code: String| {
        let app = t.app.clone();
        let ana = ana.clone();
        async move {
            call(
                &app,
                "POST",
                "/auth/email/confirm",
                Some(&ana),
                Some(json!({ "email": "new@example.com", "code": code })),
            )
            .await
        }
    };
    assert_eq!(confirm(wrong.into()).await.1["error"], "bad_code");
    // Nothing changed yet.
    let (_, acct) = call(&t.app, "GET", "/auth/account", Some(&ana), None).await;
    assert_eq!(acct["email"], "ana@example.com");
    let (s, b) = confirm(code).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let (_, acct) = call(&t.app, "GET", "/auth/account", Some(&ana), None).await;
    assert_eq!(
        (&acct["email"], &acct["email_verified"]),
        (&json!("new@example.com"), &json!(true))
    );
    // The old address is told, with the new one masked.
    let notices = wait_notices(&t.mail, "ana@example.com", 1).await;
    assert_eq!(notices, ["Adresse e-mail modifiée"]);
    let n = t.mail.notices.lock().unwrap()[0].clone();
    assert!(n.text.contains("n***@example.com") && !n.text.contains("new@example.com"));
    assert_eq!(
        login(&t, "new@example.com", PASSWORD).await.0,
        StatusCode::OK
    );
    assert_eq!(
        login(&t, "ana@example.com", PASSWORD).await.0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn an_unverified_email_is_only_freed_by_a_proof() {
    let t = setup().await;
    let (typo, typo_id) = signup(&t, "real@example.com", "Typo").await;
    let set: Option<String> = sqlx::query_scalar("SELECT email_set_at FROM users WHERE id = ?")
        .bind(typo_id)
        .fetch_one(&t.state.db)
        .await
        .unwrap();
    assert!(set.is_some());
    let again = || {
        call(
            &t.app,
            "POST",
            "/auth/signup",
            None,
            Some(json!({ "email": "real@example.com", "password": PASSWORD, "name": "Real" })),
        )
    };
    assert_eq!(again().await.1["error"], "email_taken");
    sqlx::query(
        "UPDATE users SET email_set_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-8 days') WHERE id = ?",
    )
    .bind(typo_id)
    .execute(&t.state.db)
    .await
    .unwrap();
    let email_of = |id: i64| {
        let db = t.state.db.clone();
        async move {
            sqlx::query_scalar::<_, Option<String>>("SELECT email FROM users WHERE id = ?")
                .bind(id)
                .fetch_one(&db)
                .await
                .unwrap()
        }
    };

    // A week later, its owner comes back on a new phone: the lookup changes nothing, and the
    // password still signs in (it used to detach the address and orphan the account).
    for _ in 0..3 {
        let (s, b) = call(
            &t.app,
            "POST",
            "/auth/email",
            None,
            Some(json!({ "email": "real@example.com" })),
        )
        .await;
        assert_eq!((s, &b["account"]), (StatusCode::OK, &json!(true)));
    }
    assert_eq!(email_of(typo_id).await.as_deref(), Some("real@example.com"));
    let (s, b) = login(&t, "real@example.com", PASSWORD).await;
    assert_eq!(
        (s, b["user_id"].as_i64()),
        (StatusCode::OK, Some(typo_id)),
        "{b}"
    );
    // A sign-up proves nothing either.
    assert_eq!(again().await.1["error"], "email_taken");
    assert_eq!(email_of(typo_id).await.as_deref(), Some("real@example.com"));

    // Someone who proves the address (a code sent to it) takes it: the account that typed it
    // keeps its sessions and can add another address.
    let (real, real_id) = signup(&t, "real-other@example.com", "Real").await;
    reauth(&t, &real).await;
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/email/change",
        Some(&real),
        Some(json!({ "email": "real@example.com" })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    // Asking for the code is not a proof.
    assert_eq!(email_of(typo_id).await.as_deref(), Some("real@example.com"));
    let code = t.mail.last_code("real@example.com").unwrap();
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/email/confirm",
        Some(&real),
        Some(json!({ "email": "real@example.com", "code": code })),
    )
    .await;
    assert_eq!(
        (s, &b["email_verified"]),
        (StatusCode::OK, &json!(true)),
        "{b}"
    );
    assert_eq!(email_of(typo_id).await, None);
    assert_eq!(email_of(real_id).await.as_deref(), Some("real@example.com"));
    let (s, acct) = call(&t.app, "GET", "/auth/account", Some(&typo), None).await;
    assert_eq!((s, &acct["email"]), (StatusCode::OK, &Value::Null));

    // A verified address is never freed.
    sqlx::query("UPDATE users SET email_set_at = '2020-01-01T00:00:00.000Z' WHERE id = ?")
        .bind(real_id)
        .execute(&t.state.db)
        .await
        .unwrap();
    assert_eq!(again().await.1["error"], "email_taken");
    let (other, _) = signup(&t, "other@example.com", "Other").await;
    reauth(&t, &other).await;
    let (_, b) = call(
        &t.app,
        "POST",
        "/auth/email/change",
        Some(&other),
        Some(json!({ "email": "real@example.com" })),
    )
    .await;
    assert_eq!(b["error"], "email_taken");
}

#[tokio::test]
async fn lost_2fa_without_codes_is_reset_by_email_a_day_later() {
    let t = setup().await;
    let (ana, ana_id) = signup(&t, "ana@example.com", "Ana").await;
    verify_email(&t, &ana, "ana@example.com").await;
    let secret = enable_2fa(&t, &ana).await;
    let challenge = || async {
        let (s, b) = login(&t, "ana@example.com", PASSWORD).await;
        assert_eq!(s, StatusCode::OK, "{b}");
        b["challenge"].as_str().unwrap().to_owned()
    };
    let c = challenge().await;
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/2fa/reset/send",
        None,
        Some(json!({ "challenge": "nope" })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_code"))
    );
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/2fa/reset/send",
        None,
        Some(json!({ "challenge": c })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["email"], "a***@example.com");
    let code = t.mail.last_code("ana@example.com").unwrap();
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/2fa/reset/confirm",
        None,
        Some(json!({ "challenge": c, "code": code })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let at = b["reset_at"].as_i64().unwrap();
    assert!((at - now() as i64 - 24 * 3600).abs() < 60, "{at}");
    // The notices are sent from spawned tasks: their order isn't fixed.
    assert!(
        wait_notices(&t.mail, "ana@example.com", 2)
            .await
            .iter()
            .any(|n| n.contains("retrait demandé"))
    );
    let (_, acct) = call(&t.app, "GET", "/auth/account", Some(&ana), None).await;
    assert_eq!(acct["mfa_reset_at"], at);
    // Before the day is over, 2FA still applies.
    assert!(login(&t, "ana@example.com", PASSWORD).await.1["challenge"].is_string());

    // A signed-in session cancels it.
    let (s, _) = call(&t.app, "POST", "/auth/2fa/reset/cancel", Some(&ana), None).await;
    assert_eq!(s, StatusCode::OK);
    let (_, acct) = call(&t.app, "GET", "/auth/account", Some(&ana), None).await;
    assert_eq!(acct["mfa_reset_at"], Value::Null);

    // Asked again, and the day is over: the next sign-in removes 2FA.
    let c = challenge().await;
    call(
        &t.app,
        "POST",
        "/auth/2fa/reset/send",
        None,
        Some(json!({ "challenge": c })),
    )
    .await;
    let code = t.mail.last_code("ana@example.com").unwrap();
    let (s, _) = call(
        &t.app,
        "POST",
        "/auth/2fa/reset/confirm",
        None,
        Some(json!({ "challenge": c, "code": code })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    sqlx::query("UPDATE users SET mfa_reset_at = ? WHERE id = ?")
        .bind(now() as i64 - 1)
        .bind(ana_id)
        .execute(&t.state.db)
        .await
        .unwrap();
    let (s, b) = login(&t, "ana@example.com", PASSWORD).await;
    assert_eq!(s, StatusCode::OK);
    let token = b["token"]
        .as_str()
        .expect("signed in without a code")
        .to_owned();
    let (_, acct) = call(&t.app, "GET", "/auth/account", Some(&token), None).await;
    assert_eq!(
        (&acct["totp"], &acct["recovery_codes_left"]),
        (&json!(false), &json!(0))
    );
    let notices = wait_notices(&t.mail, "ana@example.com", 4).await;
    assert!(
        notices
            .iter()
            .any(|n| n == "Double authentification retirée"),
        "{notices:?}"
    );
    let _ = secret;

    // Signing in with a code calls a pending reset off; no verified e-mail, no reset.
    let secret = enable_2fa(&t, &token).await;
    let c = challenge().await;
    call(
        &t.app,
        "POST",
        "/auth/2fa/reset/send",
        None,
        Some(json!({ "challenge": c })),
    )
    .await;
    let code = t.mail.last_code("ana@example.com").unwrap();
    call(
        &t.app,
        "POST",
        "/auth/2fa/reset/confirm",
        None,
        Some(json!({ "challenge": c, "code": code })),
    )
    .await;
    let c = challenge().await;
    let (s, _) = call(
        &t.app,
        "POST",
        "/auth/login/2fa",
        None,
        Some(json!({ "challenge": c, "code": totp_code(&secret, now() + 30).unwrap() })),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, acct) = call(&t.app, "GET", "/auth/account", Some(&token), None).await;
    assert_eq!(acct["mfa_reset_at"], Value::Null);
    sqlx::query("UPDATE users SET email_verified_at = NULL WHERE id = ?")
        .bind(ana_id)
        .execute(&t.state.db)
        .await
        .unwrap();
    let c = challenge().await;
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/2fa/reset/send",
        None,
        Some(json!({ "challenge": c })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("no_email"))
    );
}

#[tokio::test]
async fn sessions_are_capped_and_expire_when_unused() {
    let t = setup().await;
    let (first, ana_id) = signup(&t, "ana@example.com", "Ana").await;
    for _ in 0..20 {
        assert_eq!(
            login(&t, "ana@example.com", PASSWORD).await.0,
            StatusCode::OK
        );
    }
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_tokens WHERE user_id = ?")
        .bind(ana_id)
        .fetch_one(&t.state.db)
        .await
        .unwrap();
    assert_eq!(n, quidenous_server::accounts::MAX_SESSIONS);
    // The least recently used one (the sign-up's, never used since) went.
    assert_eq!(
        call(&t.app, "GET", "/auth/account", Some(&first), None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );

    // 180 days unused: gone for an account that can sign in again, kept for a legacy one.
    let (_, b) = login(&t, "ana@example.com", PASSWORD).await;
    let ana = b["token"].as_str().unwrap().to_owned();
    let (_, b) = legacy_join(&t.app, "Rita").await;
    let rita = b["token"].as_str().unwrap().to_owned();
    sqlx::query(
        "UPDATE user_tokens SET last_used_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-181 days'), \
         created_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-200 days')",
    )
    .execute(&t.state.db)
    .await
    .unwrap();
    let purged = quidenous_server::accounts::purge_idle_sessions(&t.state)
        .await
        .unwrap();
    assert_eq!(purged, 20);
    assert_eq!(
        call(&t.app, "GET", "/auth/account", Some(&ana), None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(&t.app, "GET", "/me", Some(&rita), None).await.0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn a_claimed_session_can_secure_an_account_whose_other_device_is_long_unused() {
    let t = setup().await;
    let (_, b) = legacy_join(&t.app, "Rita").await;
    let lost = b["token"].as_str().unwrap().to_owned();
    let (_, b) = legacy_join(&t.app, "rita").await;
    let grab = b["token"].as_str().unwrap().to_owned();
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/sessions/revoke-others",
        Some(&grab),
        None,
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("claimed_session"))
    );
    // The lost phone hasn't been used for 31 days.
    let hash = hex::encode(Sha256::digest(lost.as_bytes()));
    sqlx::query(
        "UPDATE user_tokens SET last_used_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-31 days'), \
         created_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-40 days') WHERE token_hash = ?",
    )
    .bind(hash)
    .execute(&t.state.db)
    .await
    .unwrap();
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/sessions/revoke-others",
        Some(&grab),
        None,
    )
    .await;
    assert_eq!((s, &b["revoked"]), (StatusCode::OK, &json!(1)));
    assert_eq!(
        call(&t.app, "GET", "/me", Some(&lost), None).await.0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn a_password_reset_by_email_holds_off_a_2fa_reset_for_a_week() {
    let t = setup().await;
    let (ana, ana_id) = signup(&t, "ana@example.com", "Ana").await;
    verify_email(&t, &ana, "ana@example.com").await;
    let secret = enable_2fa(&t, &ana).await;
    let mfa_reset =
        async |path: &str, body: Value| call(&t.app, "POST", path, None, Some(body)).await;

    // A 2FA reset is pending (confirmed), and another code is on its way. (Sends share the
    // 3 / hour budget with `forgot`.)
    sqlx::query("UPDATE users SET mfa_reset_at = ? WHERE id = ?")
        .bind(now() as i64 + 3600)
        .bind(ana_id)
        .execute(&t.state.db)
        .await
        .unwrap();
    let (_, b) = login(&t, "ana@example.com", PASSWORD).await;
    let c = b["challenge"].as_str().unwrap().to_owned();
    let (s, _) = mfa_reset("/auth/2fa/reset/send", json!({ "challenge": c })).await;
    assert_eq!(s, StatusCode::OK);
    let pending = t.mail.last_code("ana@example.com").unwrap();

    // With the mailbox alone: the password is reset by e-mail...
    let code = forgot_code(&t, "ana@example.com").await;
    let new_password = "nouveau-mot-de-passe-solide";
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/reset",
        None,
        Some(json!({ "email": "ana@example.com", "code": code, "password": new_password })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let c = b["challenge"]
        .as_str()
        .expect("2FA still applies")
        .to_owned();
    // ...which calls the pending 2FA reset off, its codes too...
    let at: Option<i64> = sqlx::query_scalar("SELECT mfa_reset_at FROM users WHERE id = ?")
        .bind(ana_id)
        .fetch_one(&t.state.db)
        .await
        .unwrap();
    assert_eq!(at, None);
    let (s, b) = mfa_reset(
        "/auth/2fa/reset/confirm",
        json!({ "challenge": c, "code": pending }),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("password_reset_recent"))
    );
    // ...and no new 2FA reset by e-mail can start for a week.
    let mails = t.mail.count("ana@example.com");
    let (s, b) = mfa_reset("/auth/2fa/reset/send", json!({ "challenge": c })).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("password_reset_recent"))
    );
    assert_eq!(t.mail.count("ana@example.com"), mails, "no code sent");
    // The owner, with the second factor, still signs in.
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/login/2fa",
        None,
        Some(json!({ "challenge": c, "code": totp_code(&secret, now() + 30).unwrap() })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");

    // A week later, the e-mail way works again (and the old code is still dead).
    sqlx::query("UPDATE users SET password_reset_at = ? WHERE id = ?")
        .bind(now() as i64 - 7 * 24 * 3600 - 60)
        .bind(ana_id)
        .execute(&t.state.db)
        .await
        .unwrap();
    let (_, b) = login(&t, "ana@example.com", new_password).await;
    let c = b["challenge"].as_str().unwrap().to_owned();
    let (s, b) = mfa_reset(
        "/auth/2fa/reset/confirm",
        json!({ "challenge": c, "code": pending }),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    let (s, b) = mfa_reset("/auth/2fa/reset/send", json!({ "challenge": c })).await;
    assert_eq!(s, StatusCode::OK, "{b}");
}

#[tokio::test]
async fn tickets_are_capped_per_session_account_and_ip() {
    use quidenous_server::{
        accounts::tickets::MAX_PER_SESSION,
        platform::quotas::{TICKET_IP, TICKET_USER},
    };
    let t = setup().await;
    let (ana, _) = signup(&t, "ana@example.com", "Ana").await;
    let room = room_with(&t, &ana, &[]).await;
    // One session keeps its last `MAX_PER_SESSION` live tickets: asking for more drops the oldest.
    let mut tickets = Vec::new();
    for _ in 0..MAX_PER_SESSION + 5 {
        let (s, b) = ticket(&t, &ana, "10.0.0.1").await;
        assert_eq!(s, StatusCode::OK, "{b}");
        tickets.push(b["ticket"].as_str().unwrap().to_owned());
    }
    let export = async |tk: &str| {
        send(
            &t.app,
            "GET",
            &format!("/history/export?ticket={tk}&room={room}"),
            None,
            None,
            &[],
        )
        .await
        .status
    };
    for tk in &tickets[..5] {
        assert_eq!(export(tk).await, StatusCode::UNAUTHORIZED);
    }
    for tk in &tickets[5..] {
        assert_eq!(export(tk).await, StatusCode::OK);
    }
    // Per account, whatever the session.
    let (s, b) = login(&t, "ana@example.com", PASSWORD).await;
    assert_eq!(s, StatusCode::OK);
    let other = b["token"].as_str().unwrap().to_owned();
    let mut asked = tickets.len();
    let status = loop {
        let (s, _) = ticket(&t, &other, "10.0.0.2").await;
        asked += 1;
        if s != StatusCode::OK || asked > TICKET_USER.capacity as usize + 3 {
            break s;
        }
    };
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    // (A slow run may have refilled a token meanwhile.)
    let cap = TICKET_USER.capacity as usize;
    assert!((cap + 1..=cap + 2).contains(&asked), "{asked}");
    // Per IP: another account behind a busy IP.
    let (bob, _) = signup(&t, "bob@example.com", "Bob").await;
    for _ in 0..TICKET_IP.capacity {
        t.state.auth.limits.hit(&TICKET_IP, "10.0.0.3").unwrap();
    }
    let (s, b) = ticket(&t, &bob, "10.0.0.3").await;
    assert_eq!(s, StatusCode::TOO_MANY_REQUESTS, "{b}");
    let (s, _) = ticket(&t, &bob, "10.0.0.4").await;
    assert_eq!(s, StatusCode::OK);
}

#[tokio::test]
async fn a_suspended_account_gets_and_keeps_no_session() {
    let t = setup().await;
    // A session left over (issued while the suspension was being written) is no session.
    let (ana, ana_id) = signup(&t, "ana@example.com", "Ana").await;
    let room = room_with(&t, &ana, &[]).await;
    let (s, _) = in_room(&t, "GET", "/me", &ana, room, None).await;
    assert_eq!(s, StatusCode::OK);
    let (s, b) = ticket(&t, &ana, "10.0.0.1").await;
    assert_eq!(s, StatusCode::OK);
    let tk = b["ticket"].as_str().unwrap().to_owned();
    suspend_in_db(&t, ana_id).await;
    for path in ["/me", "/group", "/rooms"] {
        let (s, _) = in_room(&t, "GET", path, &ana, room, None).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED, "{path}");
    }
    let (s, _) = call(&t.app, "GET", "/auth/account", Some(&ana), None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (s, _) = ticket(&t, &ana, "10.0.0.1").await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let r = send(
        &t.app,
        "GET",
        &format!("/history/export?ticket={tk}&room={room}"),
        None,
        None,
        &[],
    )
    .await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);

    // Suspended between the password and the 2FA code: no session either.
    let (bob, bob_id) = signup(&t, "bob@example.com", "Bob").await;
    verify_email(&t, &bob, "bob@example.com").await;
    let secret = enable_2fa(&t, &bob).await;
    let (_, b) = login(&t, "bob@example.com", PASSWORD).await;
    let c = b["challenge"].as_str().unwrap().to_owned();
    suspend_in_db(&t, bob_id).await;
    let (s, b) = call(
        &t.app,
        "POST",
        "/auth/login/2fa",
        None,
        Some(json!({ "challenge": c, "code": totp_code(&secret, now() + 30).unwrap() })),
    )
    .await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("suspended")),
        "{b}"
    );
    assert!(b["token"].is_null());

    // A legacy account can't come back by typing its first name.
    let (s, b) = legacy_join(&t.app, "Zoé").await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let zoe = b["token"].as_str().unwrap().to_owned();
    let zoe_id: i64 = sqlx::query_scalar("SELECT user_id FROM user_tokens WHERE token_hash = ?")
        .bind(hex::encode(Sha256::digest(zoe.as_bytes())))
        .fetch_one(&t.state.db)
        .await
        .unwrap();
    suspend_in_db(&t, zoe_id).await;
    sqlx::query("DELETE FROM user_tokens WHERE user_id = ?")
        .bind(zoe_id)
        .execute(&t.state.db)
        .await
        .unwrap();
    let (s, b) = legacy_join(&t.app, "zoé").await;
    assert_eq!(
        (s, b["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("suspended")),
        "{b}"
    );
    let tokens: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_tokens WHERE user_id = ?")
        .bind(zoe_id)
        .fetch_one(&t.state.db)
        .await
        .unwrap();
    assert_eq!(tokens, 0);
}
