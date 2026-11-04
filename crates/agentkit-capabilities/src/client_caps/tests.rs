#![allow(clippy::unwrap_used)]
use super::*;
use agentkit_protocol::RpcValue;
fn arg(value: serde_json::Value) -> Vec<RpcValue> {
    vec![serde_json::from_value(value).unwrap()]
}
#[test]
fn external_urls_allow_only_the_three_supported_schemes() {
    for url in [
        "http://localhost/test",
        "https://example.com/a?b=c",
        "mailto:user@example.com",
    ] {
        assert!(matches!(
            parse(NAMES[0], arg(serde_json::json!(url))),
            Ok(Action::OpenExternal(_))
        ));
    }
    for url in [
        "javascript:alert(1)",
        "file:///tmp/foo",
        "data:text/plain,a",
        "shipagent://run",
        "https://a.test/\nother",
    ] {
        assert!(parse(NAMES[0], arg(serde_json::json!(url))).is_err());
    }
}
#[test]
fn local_paths_reject_urls_and_relative_or_nul_paths() {
    let temp = tempfile::tempdir().unwrap();
    assert!(matches!(
        parse(NAMES[1], arg(serde_json::json!(temp.path()))),
        Ok(Action::OpenPath(_))
    ));
    for path in ["relative.txt", "https://example.com", "bad\0file"] {
        assert!(parse(NAMES[2], arg(serde_json::json!(path))).is_err());
    }
}
#[test]
fn confirmations_preserve_indices_and_validate_the_whole_spec() {
    let value = serde_json::json!({"title":"Title","message":"Message","buttons":["Delete","Keep","Inspect"],"defaultId":2,"cancelId":1,"type":"warning"});
    let Action::Confirm(spec) = parse(NAMES[3], arg(value)).unwrap() else {
        panic!()
    };
    assert_eq!((spec.default_index(), spec.cancel_index()), (2, 1));
    for value in [
        serde_json::json!({"title":"t","message":"m","buttons":[]}),
        serde_json::json!({"title":"t","message":"m","buttons":["OK"],"defaultId":2}),
        serde_json::json!({"title":"t","message":"m","buttons":["OK"],"cancelId":-1}),
    ] {
        assert!(parse(NAMES[3], arg(value)).is_err());
    }
}
#[test]
fn file_properties_keep_file_directory_multiselect_and_filters_distinct() {
    for (properties, files, dirs, multiple) in [
        (vec![], true, false, false),
        (vec!["openDirectory"], false, true, false),
        (
            vec!["openFile", "openDirectory", "multiSelections"],
            true,
            true,
            true,
        ),
    ] {
        let Action::OpenFile(spec)=parse(NAMES[4],arg(serde_json::json!({"properties":properties,"filters":[{"name":"Images","extensions":["png","jpg"]}]}))).unwrap() else {panic!()};
        assert_eq!(
            (spec.files(), spec.directories(), spec.multiple()),
            (files, dirs, multiple)
        );
        assert_eq!(spec.filters[0].extensions, ["png", "jpg"]);
    }
    assert!(
        parse(
            NAMES[4],
            arg(serde_json::json!({"properties":["saveFile"]}))
        )
        .is_err()
    );
}
#[test]
fn unsupported_and_malformed_calls_return_protocol_errors() {
    assert_eq!(
        parse("client:browser:invoke", arg(serde_json::json!({})))
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnavailable
    );
    assert_eq!(
        parse(NAMES[0], vec![]).unwrap_err().code,
        ErrorCode::HandlerError
    );
}
#[test]
fn bridge_returns_results_and_cancels_requests_when_the_handler_future_is_dropped() {
    smol::block_on(async {
        let (handlers, requests) = bridge::channel();
        assert_eq!(handlers.len(), 5);
        assert!(!handlers.contains_key("client:browser:invoke"));
        let mut pending = handlers[NAMES[0]](arg(serde_json::json!("https://example.com")));
        assert!(smol::future::poll_once(&mut pending).await.is_none());
        let request = requests.recv().await.unwrap();
        assert!(request.live());
        drop(pending);
        assert!(!request.live());
        assert!(request.canceled.try_recv().is_ok());
        let mut pending =
            handlers[NAMES[1]](arg(serde_json::json!(tempfile::tempdir().unwrap().path())));
        assert!(smol::future::poll_once(&mut pending).await.is_none());
        let request = requests.recv().await.unwrap();
        request.finish(Ok(RpcValue::Null));
        assert_eq!(pending.await.unwrap(), RpcValue::Null);
    });
}
#[test]
fn file_selection_cancellation_and_unicode_paths_have_the_exact_wire_shape() {
    let value = files::result(None).unwrap();
    assert_eq!(
        serde_json::to_value(value).unwrap(),
        serde_json::json!({"canceled":true,"filePaths":[]})
    );
    let value = files::result(Some(vec![std::path::PathBuf::from("中文 file.txt")])).unwrap();
    assert_eq!(
        serde_json::to_value(value).unwrap(),
        serde_json::json!({"canceled":false,"filePaths":["中文 file.txt"]})
    );
}
