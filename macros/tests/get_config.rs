use bagua::configs::GetConfig;

#[derive(bagua::GetConfig)]
pub struct Settings {
    storage: StorageCfg,
    aa: Aa,
}

pub struct Aa {
    a: u32,
}

pub struct StorageCfg {
    root: String,
}

#[test]
fn t_get_config() {
    let settings = Settings {
        storage: StorageCfg {
            root: "aa".to_string(),
        },
        aa: Aa { a: 1 },
    };
    let storage_cfg: &StorageCfg = settings.get_config();
    assert_eq!(storage_cfg.root, "aa");

    let a: &Aa = settings.get_config();
    assert_eq!(a.a, 1);
}
