use serde::Deserialize;

use super::FieldGroup;

pub trait Updater {
    type FieldGroup: FieldGroup;
}

pub fn de_double_option<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    Deserialize::deserialize(deserializer).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Deserialize, Default)]
    pub struct Updater {
        #[serde(deserialize_with = "de_double_option")]
        #[serde(default)]
        pub name: Option<String>,
        #[serde(deserialize_with = "de_double_option")]
        #[serde(default)]
        pub age: Option<Option<u8>>,
        #[serde(deserialize_with = "de_double_option")]
        #[serde(default)]
        pub email: Option<String>,
    }

    #[test]
    fn test_deserialize_optional() {
        let json = r#"{"name": "John", "age": null}"#;
        let updater: Updater = serde_json::from_str(json).unwrap();
        assert_eq!(updater.name, Some("John".to_string()));
        assert_eq!(updater.age, Some(None));
        assert_eq!(updater.email, None);
    }
}
