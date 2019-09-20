use std::{fmt, iter::FromIterator, marker::PhantomData};

use derive_more::Constructor;
use protocol::codec::ProtocolCodecSync;
use serde::{de, ser::SerializeStruct, Deserializer, Serializer};
use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct TWrapper<T: ProtocolCodecSync> {
    #[serde(with = "super::serde")]
    inner: T,
}

#[derive(Constructor, Serialize)]
struct VecT<T: ProtocolCodecSync> {
    inner: Vec<TWrapper<T>>,
}

pub fn serialize<'se, V, T, S>(val: &'se V, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    V: IntoIterator<Item = T> + Clone,
    T: ProtocolCodecSync + 'se + Clone,
{
    let val_cloned = val.clone().into_iter();
    let inner = val_cloned
        .map(|t| TWrapper { inner: t })
        .collect::<Vec<_>>();

    let vec_t = VecT { inner };

    let mut state = s.serialize_struct("VecT", 1)?;
    state.serialize_field("inner", &vec_t.inner)?;
    state.end()
}

pub fn deserialize<'de, T, V, D>(deserializer: D) -> Result<V, D::Error>
where
    D: Deserializer<'de>,
    V: FromIterator<T>,
    T: ProtocolCodecSync,
{
    #[derive(Deserialize)]
    #[serde(field_identifier, rename_all = "lowercase")]
    enum Field {
        Inner,
    }

    struct VecTVisitor<T> {
        pin_t: PhantomData<T>,
    }

    impl<T> VecTVisitor<T> {
        pub fn new() -> Self {
            VecTVisitor { pin_t: PhantomData }
        }
    }

    impl<'de, T> de::Visitor<'de> for VecTVisitor<T>
    where
        T: ProtocolCodecSync,
    {
        type Value = VecT<T>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("serde multi")
        }

        fn visit_seq<V>(self, mut seq: V) -> Result<Self::Value, V::Error>
        where
            V: de::SeqAccess<'de>,
        {
            let inner = seq
                .next_element()?
                .ok_or_else(|| de::Error::invalid_length(0, &self))?;

            Ok(VecT::new(inner))
        }

        fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
        where
            V: de::MapAccess<'de>,
        {
            let mut inner = None;

            while let Some(key) = map.next_key()? {
                match key {
                    Field::Inner => {
                        if inner.is_some() {
                            return Err(de::Error::duplicate_field("inner"));
                        }
                        inner = Some(map.next_value()?);
                    }
                }
            }

            let inner = inner.ok_or_else(|| de::Error::missing_field("inner"))?;
            Ok(VecT::new(inner))
        }
    }

    const FIELDS: &[&str] = &["inner"];
    let vec_t = deserializer.deserialize_struct("VecT", FIELDS, VecTVisitor::new())?;

    Ok(V::from_iter(
        vec_t.inner.into_iter().map(|wrap_t| wrap_t.inner),
    ))
}
