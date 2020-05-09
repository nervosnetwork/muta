use std::collections::{BTreeMap, HashMap};

use protocol::types::{ChainSchema, DataMeta, MethodMeta, ServiceMeta, ServiceSchema};

pub fn lazy_schema(metas: HashMap<String, ServiceMeta>) -> ChainSchema {
    let mut service_schemas = vec![];
    for (s, m) in metas.into_iter() {
        service_schemas.push(gen_schema(s, m));
    }
    ChainSchema {
        schema: service_schemas,
    }
}

pub fn gen_schema(service: String, meta: ServiceMeta) -> ServiceSchema {
    let method_schema = if meta.methods.is_empty() {
        "".to_owned()
    } else {
        let methods = gen_schema_methods(&meta.methods);
        let method_params = gen_schema_objects(&meta.method_params);
        methods + method_params.as_str()
    };
    let event_schema = if meta.events.is_empty() {
        "".to_owned()
    } else {
        let events = gen_schema_events(&meta.events);
        let event_structs = gen_schema_objects(&meta.event_structs);
        events + event_structs.as_str()
    };

    ServiceSchema {
        service,
        method: method_schema,
        event: event_schema,
    }
}

pub fn gen_schema_events(events: &[String]) -> String {
    let mut events_schema = "union Event = ".to_owned();
    for e in events.iter() {
        let event = e.to_owned() + " | ";
        events_schema.push_str(event.as_str());
    }
    events_schema.truncate(events_schema.len() - 3);
    events_schema + "\n\n"
}

pub fn gen_schema_methods(methods: &[MethodMeta]) -> String {
    let mut mutation = format!("type Mutation {}\n", "{");
    let mut query = format!("type Query {}\n", "{");
    let mut need_null = false;
    for m in methods.iter() {
        let payload_str = if "" == &m.payload_type {
            ":".to_owned()
        } else {
            format!("(\n    payload: {}!\n  ):", &m.payload_type)
        };
        let res_str = if "" == &m.res_type {
            " Null\n".to_owned()
        } else {
            format!(" {}!\n", &m.res_type)
        };
        let method_str = format!("  {}{}{}", &m.method_name, &payload_str, &res_str);
        if "" == &m.res_type {
            need_null = true;
        }
        if m.readonly {
            query.push_str(method_str.as_str());
        } else {
            mutation.push_str(method_str.as_str());
        }
    }
    let scalar_null = if need_null { "scalar Null\n\n" } else { "" };
    if format!("type Mutation {}\n", "{") == mutation {
        mutation = "".to_owned();
    } else {
        mutation += "}\n\n";
    }
    if format!("type Query {}\n", "{") == query {
        query = "".to_owned();
    } else {
        query += "}\n\n";
    }

    mutation + query.as_str() + scalar_null
}

pub fn gen_schema_objects(meta: &BTreeMap<String, DataMeta>) -> String {
    let mut schema = "".to_owned();
    for p in meta.values() {
        if let DataMeta::Scalar(s) = &p {
            let scalar_string = format!("{}scalar {}\n\n", &s.comment, &s.name);
            schema.push_str(scalar_string.as_str());
        } else if let DataMeta::Struct(s) = &p {
            let mut struct_string = format!("{}type {} {}\n", &s.comment, &s.name, "{");
            for f in s.fields.iter() {
                if f.is_vec {
                    let field_string = format!("{}  {}: [{}!]!\n", f.comment, f.name, f.ty);
                    struct_string.push_str(field_string.as_str());
                } else {
                    let field_string = format!("{}  {}: {}!\n", f.comment, f.name, f.ty);
                    struct_string.push_str(field_string.as_str());
                };
            }
            struct_string.push_str("}\n\n");
            schema.push_str(struct_string.as_str());
        }
    }
    schema
}
