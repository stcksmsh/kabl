use kabl_modules::registry::{all_infos, create, info_for, KNOWN_KINDS};

#[test]
fn creates_every_known_kind() {
    for &kind in KNOWN_KINDS {
        let module = create(kind);
        assert!(
            module.is_some(),
            "registry should build a `{kind}` instance"
        );
        assert_eq!(
            module.unwrap().info().kind,
            kind,
            "built instance should report its own kind"
        );
    }
}

#[test]
fn unknown_kind_returns_none_not_a_panic() {
    assert!(create("nonexistent.kind").is_none());
}

#[test]
fn all_infos_covers_exactly_the_known_kinds() {
    let info_kinds: Vec<&str> = all_infos().iter().map(|info| info.kind).collect();
    assert_eq!(info_kinds.len(), KNOWN_KINDS.len());
    for &kind in KNOWN_KINDS {
        assert!(info_kinds.contains(&kind), "all_infos() missing `{kind}`");
    }
}

#[test]
fn info_for_matches_the_built_instance() {
    for &kind in KNOWN_KINDS {
        let info = info_for(kind).unwrap_or_else(|| panic!("no info for `{kind}`"));
        let instance = create(kind).unwrap();
        assert_eq!(info.kind, instance.info().kind);
        // Same static ModuleInfo, not a copy — pointer equality is meaningful here since
        // ModuleInfo is always a `&'static` reference to one static per kind.
        assert!(std::ptr::eq(info, instance.info()));
    }
}
