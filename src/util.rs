pub mod serde_duration {
    use std::str::FromStr;
    use std::time::Duration;

    use serde::{Deserialize, Deserializer, Serializer};
    use serde::de::Error;

    pub fn deserialize<'de, D>(de: D) -> Result<Duration, D::Error>
        where
            D: Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(de)?;
        let duration = humantime::Duration::from_str(&s).map_err(Error::custom)?;
        Ok(duration.into())
    }

    #[allow(unused)]
    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
    {
        let duration: humantime::Duration = duration.clone().into();
        serializer.serialize_str(&duration.to_string())
    }
}

pub mod serde_duration_optional {
    use std::str::FromStr;
    use std::time::Duration;

    use serde::{Deserialize, Deserializer};
    use serde::de::Error;

    #[allow(dead_code)]
    pub fn deserialize<'de, D>(de: D) -> Result<Option<Duration>, D::Error>
        where
            D: Deserializer<'de>,
    {
        let s: Option<String> = Deserialize::deserialize(de)?;
        if let Some(s) = s {
            let duration = humantime::Duration::from_str(&s).map_err(Error::custom)?;
            Ok(Some(duration.into()))
        } else {
            Ok(None)
        }
    }
}