pub trait ToJson {
    fn to_json_str(&self) -> serde_json::Result<String>;

    fn to_json(&self) -> serde_json::Result<serde_json::Value>;
}

impl<T> ToJson for T
where
    T: serde::Serialize,
{
    fn to_json_str(&self) -> serde_json::Result<String> {
        serde_json::to_string(self)
    }

    fn to_json(&self) -> serde_json::Result<serde_json::Value> {
        serde_json::to_value(self)
    }
}

pub trait FromJson {
    fn from_json_str(json: &str) -> serde_json::Result<Self>
    where
        Self: Sized;

    fn from_json(json: serde_json::Value) -> serde_json::Result<Self>
    where
        Self: Sized;
}

impl<T> FromJson for T
where
    T: serde::de::DeserializeOwned,
{
    fn from_json_str(json: &str) -> serde_json::Result<Self> {
        serde_json::from_str(json)
    }

    fn from_json(json: serde_json::Value) -> serde_json::Result<Self>
    where
        Self: Sized,
    {
        serde_json::from_value(json)
    }
}
