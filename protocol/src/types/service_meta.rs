use std::collections::BTreeMap;

#[derive(Debug, PartialEq, Eq)]
pub struct ServiceMeta {
    pub methods:       Vec<MethodMeta>,
    pub events:        Vec<String>,
    pub method_params: BTreeMap<String, DataMeta>,
    pub event_structs: BTreeMap<String, DataMeta>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct MethodMeta {
    pub method_name:  String,
    pub payload_type: String,
    pub readonly:     bool,
    pub res_type:     String,
}
#[derive(Debug, PartialEq, Eq)]
pub enum DataMeta {
    Struct(StructMeta),
    Scalar(ScalarMeta),
}
#[derive(Debug, PartialEq, Eq)]
pub struct StructMeta {
    pub name:    String,
    pub fields:  Vec<FieldMeta>,
    pub comment: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ScalarMeta {
    pub name:    String,
    pub comment: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct FieldMeta {
    pub name:    String,
    pub ty:      String,
    pub is_vec:  bool,
    pub comment: String,
}
