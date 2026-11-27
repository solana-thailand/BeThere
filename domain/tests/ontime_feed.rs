//! Plan 050: the Ontime link an organiser pastes, and the card it feeds.

use event_checkin_domain::models::event::{
    MAX_ONTIME_FEED_URL_CHARS, normalize_ontime_feed_url, parse_ontime_feed_url, safe_ontime_feed,
};
use event_checkin_domain::models::live_agenda::{
    Finish, LiveAgenda, Pace, clock_text, time_of_day,
};

const HOST: &str = "ontime-server-rs.solana-thailand.workers.dev";

#[test]
fn the_page_link_and_the_feed_link_store_as_the_feed() {
    let want = format!("https://{HOST}/r/road-to-mainnet/public");
    for pasted in [
        format!("https://{HOST}/r/road-to-mainnet/public"),
        format!("https://{HOST}/r/road-to-mainnet/public/"),
        format!("https://{HOST}/r/road-to-mainnet/public/agenda/"),
        format!("https://{HOST}/r/road-to-mainnet/public/agenda/?lang=th"),
        format!("  https://{HOST}/r/road-to-mainnet/public/agenda?lang=en&theme=dark  "),
    ] {
        assert_eq!(
            normalize_ontime_feed_url(&pasted).as_deref(),
            Ok(want.as_str()),
            "{pasted}"
        );
    }
}

#[test]
fn blank_means_none() {
    assert_eq!(normalize_ontime_feed_url("  ").as_deref(), Ok(""));
    assert_eq!(safe_ontime_feed(""), None);
}

#[test]
fn the_socket_and_the_page_come_from_the_room() {
    let feed = parse_ontime_feed_url(&format!("https://{HOST}/r/day_1/public")).unwrap();
    assert_eq!(
        feed.socket_url(),
        format!("wss://{HOST}/public/ws?room=day_1")
    );
    assert_eq!(
        feed.agenda_url("th"),
        format!("https://{HOST}/r/day_1/public/agenda/?lang=th")
    );
}

#[test]
fn a_link_with_the_rooms_token_is_refused() {
    // the token lets its holder change the show; it must never reach a public page
    let err = normalize_ontime_feed_url(&format!(
        "https://{HOST}/r/day-1/public/agenda/?lang=th&token=abc123"
    ))
    .unwrap_err();
    assert!(err.contains("token"), "{err}");
    assert!(normalize_ontime_feed_url(&format!("https://{HOST}/r/day-1/public?token=x")).is_err());
}

#[test]
fn anything_but_an_ontime_attendee_link_is_refused() {
    for bad in [
        format!("http://{HOST}/r/day-1/public"),
        format!("javascript://{HOST}/r/day-1/public"),
        format!("https://{HOST}/r/day-1"),
        format!("https://{HOST}/r/day-1/editor"),
        format!("https://{HOST}/r/day-1/public/ws"),
        format!("https://{HOST}/r/day-1/public/agenda/extra"),
        format!("https://{HOST}/public/ws?room=day-1"),
        format!("https://{HOST}/r/day%201/public"),
        format!("https://{HOST}/r/../public"),
        format!("https://{HOST}/r/{}/public", "a".repeat(65)),
        format!("https://user@{HOST}/r/day-1/public"),
        format!("https://{HOST}:x/r/day-1/public"),
        "https:///r/day-1/public".to_string(),
        "https://.evil/r/day-1/public".to_string(),
        format!("https://{HOST}/r/day-1/public#x"),
        format!("https://{HOST}/r/day 1/public"),
        format!(
            "https://{HOST}/r/day-1/public?{}",
            "a".repeat(MAX_ONTIME_FEED_URL_CHARS)
        ),
    ] {
        assert!(normalize_ontime_feed_url(&bad).is_err(), "accepted {bad}");
        assert_eq!(safe_ontime_feed(&bad), None, "{bad}");
    }
}

#[test]
fn errors_never_carry_a_scheme() {
    // the Worker redacts error text with `://` in it
    for bad in [
        "http://x/r/a/public",
        "https://x/r/a",
        "https://x/r/a/public?token=1",
        "https://x:y/r/a/public",
    ] {
        let err = normalize_ontime_feed_url(bad).unwrap_err();
        assert!(!err.contains("://"), "{err}");
    }
}

#[test]
fn a_local_port_is_kept_and_the_host_lowercased() {
    let feed = parse_ontime_feed_url("https://Ontime.Example.com:8443/r/Day-1/public").unwrap();
    assert_eq!(
        feed.feed_url(),
        "https://ontime.example.com:8443/r/Day-1/public"
    );
}

/// A packet as ontime-rs sends it (`public_feed::agenda` and `live`), with
/// the nulls and the fields the card does not use.
fn packet(playback: &str, current: Option<i64>, offset: i64) -> String {
    let current = current.map_or("null".to_string(), |c| c.to_string());
    format!(
        r##"{{"tag":"public-data","payload":{{
          "agenda":{{"title":"Road to Mainnet","rundown":"Day 1","fields":["Presenter"],"entries":[
            {{"type":"group","id":"g","title":"Morning","colour":""}},
            {{"type":"event","id":"a","cue":"1","title":"Opening","timeStart":36000000,"timeEnd":37800000,
              "duration":1800000,"dayOffset":0,"colour":"#779BE7","group":"g","fields":{{"Presenter":"Ozone"}}}},
            {{"type":"milestone","id":"m","cue":"","title":"Doors","colour":"","group":null}},
            {{"type":"event","id":"b","cue":"2","title":"Keynote","timeStart":37800000,"timeEnd":39600000,
              "duration":1800000,"dayOffset":0,"colour":"","group":null,"fields":{{}}}}]}},
          "live":{{"playback":"{playback}","now":"a","next":"b","clock":36060000,
            "timer":{{"current":{current},"duration":1800000,"addedTime":0,"expectedFinish":37800000,"phase":"default"}},
            "offset":{{"mode":"absolute","absolute":{offset},"relative":0,"expectedEnd":39660000}},
            "plannedStart":36000000,"plannedEnd":39600000,"actualStart":36000000}}}}}}"##
    )
}

#[test]
fn what_is_on_now_and_next() {
    let mut feed = LiveAgenda::default();
    assert!(feed.apply(&packet("play", Some(1_200_000), 0), 1_000));
    let card = feed.card(1_000).expect("a card while it plays");
    assert_eq!(card.title, "Opening");
    assert_eq!(
        card.fields,
        vec![("Presenter".to_string(), "Ozone".to_string())]
    );
    assert_eq!(card.next_title.as_deref(), Some("Keynote"));
    assert_eq!(card.left_ms, Some(1_200_000));
    assert!((card.progress - 1.0 / 3.0).abs() < 1e-9);
    assert_eq!(card.pace, Pace::OnTime);
    assert_eq!(card.finish, Some(Finish::Expected(39_660_000)));
    assert_eq!(time_of_day(39_660_000), "11:01");
    assert!(!card.paused);
    assert_eq!(feed.title(), Some("Road to Mainnet"));
}

#[test]
fn it_counts_by_itself_while_playing_and_holds_while_paused() {
    let mut feed = LiveAgenda::default();
    feed.apply(&packet("play", Some(60_000), 0), 10_000);
    assert_eq!(feed.card(25_000).unwrap().left_ms, Some(45_000));
    assert_eq!(feed.card(80_000).unwrap().left_ms, Some(-10_000));
    assert_eq!(feed.card(80_000).unwrap().progress, 1.0);

    feed.apply(&packet("pause", Some(60_000), 0), 10_000);
    let card = feed.card(99_000).unwrap();
    assert!(card.paused);
    assert_eq!(card.left_ms, Some(60_000));
}

#[test]
fn nothing_running_shows_nothing() {
    for playback in ["stop", "armed", "roll", "something-new"] {
        let mut feed = LiveAgenda::default();
        assert!(feed.apply(&packet(playback, Some(1_000), 0), 0));
        assert_eq!(feed.card(0), None, "{playback}");
    }
    // half a feed is not enough either
    let mut feed = LiveAgenda::default();
    feed.apply(
        r#"{"tag":"public-data","payload":{"live":{"playback":"play","now":"a"}}}"#,
        0,
    );
    assert_eq!(feed.card(0), None);
}

#[test]
fn a_now_that_is_not_an_event_in_the_agenda_shows_nothing() {
    let mut feed = LiveAgenda::default();
    feed.apply(
        &packet("play", Some(1_000), 0).replace(r#""now":"a""#, r#""now":"m""#),
        0,
    );
    assert_eq!(feed.card(0), None);
}

#[test]
fn a_packet_with_only_live_keeps_the_agenda() {
    let mut feed = LiveAgenda::default();
    feed.apply(&packet("play", Some(1_000), 0), 0);
    let live_only = r#"{"tag":"public-data","payload":{"live":{"playback":"play","now":"b","next":null,
        "timer":{"current":5000,"duration":1800000},"offset":{"mode":"absolute","absolute":300000}}}}"#;
    assert!(feed.apply(live_only, 0));
    let card = feed.card(0).unwrap();
    assert_eq!(card.title, "Keynote");
    assert_eq!(card.next_title, None);
    assert_eq!(card.pace, Pace::Late { minutes: 5 });
    assert_eq!(card.finish, None);
}

#[test]
fn the_planned_end_stands_in_until_ontime_has_an_estimate() {
    let mut feed = LiveAgenda::default();
    feed.apply(
        &packet("play", Some(1_000), 0)
            .replace(r#""expectedEnd":39660000"#, r#""expectedEnd":null"#),
        0,
    );
    assert_eq!(
        feed.card(0).unwrap().finish,
        Some(Finish::Planned(39_600_000))
    );
}

#[test]
fn late_and_ahead_round_to_minutes() {
    assert_eq!(Pace::from_offset(59_999), Pace::OnTime);
    assert_eq!(Pace::from_offset(-59_999), Pace::OnTime);
    assert_eq!(Pace::from_offset(60_000), Pace::Late { minutes: 1 });
    assert_eq!(Pace::from_offset(449 * 60_000), Pace::Late { minutes: 449 });
    assert_eq!(Pace::from_offset(-150_000), Pace::Ahead { minutes: 3 });
}

#[test]
fn the_relative_offset_is_used_when_the_room_says_so() {
    let mut feed = LiveAgenda::default();
    let relative = packet("play", Some(1_000), 0).replace(
        r#""mode":"absolute","absolute":0,"relative":0"#,
        r#""mode":"relative","absolute":0,"relative":-120000"#,
    );
    feed.apply(&relative, 0);
    assert_eq!(feed.card(0).unwrap().pace, Pace::Ahead { minutes: 2 });
}

#[test]
fn the_get_body_reads_like_a_packet() {
    let body = packet("play", Some(1_000), 0);
    let start = body.find(r#""payload":"#).unwrap() + r#""payload":"#.len();
    let mut feed = LiveAgenda::default();
    assert!(feed.apply(&body[start..body.len() - 1], 0));
    assert!(feed.card(0).is_some());
}

#[test]
fn other_messages_and_garbage_are_ignored() {
    let mut feed = LiveAgenda::default();
    feed.apply(&packet("play", Some(1_000), 0), 0);
    assert!(!feed.apply(
        r#"{"tag":"runtime-data","payload":{"live":{"playback":"stop"}}}"#,
        0
    ));
    assert!(!feed.apply("not json", 0));
    assert!(!feed.apply("{}", 0));
    assert!(feed.card(0).is_some());
    feed.clear();
    assert_eq!(feed.card(0), None);
}

#[test]
fn clock_text_reads_like_a_timer() {
    assert_eq!(clock_text(1_800_000), "30:00");
    assert_eq!(clock_text(1_799_400), "30:00");
    assert_eq!(clock_text(59_000), "0:59");
    assert_eq!(clock_text(3_723_000), "1:02:03");
    assert_eq!(clock_text(0), "0:00");
    assert_eq!(clock_text(-12_000), "-0:12");
    assert_eq!(time_of_day(37_800_000), "10:30");
    assert_eq!(time_of_day(-60_000), "23:59");
}

/// A field the room left empty, a null id, a null kind or a null entry list
/// must not throw away the whole packet (review of pull 308).
#[test]
fn nulls_inside_the_agenda_keep_the_rest_of_the_packet() {
    let text = packet("play", Some(1_200_000), 0)
        .replace(
            r#""fields":{"Presenter":"Ozone"}"#,
            r#""fields":{"Presenter":null,"Room":"A"}"#,
        )
        .replace(r#"{"type":"group","id":"g""#, r#"{"type":null,"id":null"#);
    let mut feed = LiveAgenda::default();
    assert!(feed.apply(&text, 1_000), "the packet is taken");
    let card = feed.card(1_000).expect("a card while it plays");
    assert_eq!(card.title, "Opening");
    assert_eq!(card.fields, vec![("Room".to_string(), "A".to_string())]);

    let empty = r#"{"tag":"public-data","payload":{"agenda":{"title":"T","entries":null}}}"#;
    let mut feed = LiveAgenda::default();
    assert!(
        feed.apply(empty, 1_000),
        "a null entry list is an empty one"
    );
    assert_eq!(feed.title(), Some("T"));
}

/// `WebSocket::new` throws on a port it cannot use, so such a link would
/// only ever fail in the browser.
#[test]
fn a_port_the_browser_refuses_is_refused() {
    for port in ["99999", "65536", "0", "+80"] {
        let url = format!("https://{HOST}:{port}/r/day1/public");
        assert!(parse_ontime_feed_url(&url).is_err(), "{url}");
    }
    assert!(parse_ontime_feed_url(&format!("https://{HOST}:65535/r/day1/public")).is_ok());
}
