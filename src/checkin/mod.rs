mod coherent_feature_flags;
mod data;
mod feature;
mod server_options;
pub(crate) use coherent_feature_flags::CoherentFeatureFlags;
pub(crate) use data::Checkin;
pub(crate) use feature::Feature;
pub(crate) use server_options::ServerOptions;

#[cfg(test)]
mod test {
    #[test]
    fn test_parse() {
        let json = r#"
        {

            "options": {
                "dni-det-msg-ptr": {
                    "variant": "a",
                    "payload": "\"dni-det-msg-a\""
                },
                "dni-det-msg-a": {
                    "variant": "a",
                    "payload": "\"hello\""
                },
                "fine-grained-tokens": {
                    "variant": false
                }
            }
        }"#;

        let _: super::Checkin = serde_json::from_str(json).unwrap();
    }
}
