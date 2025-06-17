use liquid::Error;
use liquid_core::{Display_filter, Filter, FilterReflection, ParseFilter};
use liquid_core::{Result, Runtime};
use liquid_core::{Value, ValueView};

#[derive(Clone, ParseFilter, FilterReflection)]
#[filter(
    name = "json",
    description = "Output the raw input unescaped.",
    parsed(JSONFilter)
)]
pub struct JSON;

#[derive(Debug, Default, Display_filter)]
#[name = "json"]
struct JSONFilter;

impl Filter for JSONFilter {
    fn evaluate(&self, input: &dyn ValueView, _runtime: &dyn Runtime) -> Result<Value> {
        let serde_value = input.to_value();

        // Now, serialize the serde_json::Value to a JSON string
        let json_string = serde_json::to_string_pretty(&serde_value)
            .map_err(|e| Error::with_msg(format!("Failed to serialize to JSON: {}", e)))?;

        println!("JSON DUMP: {}", &json_string);

        // Return the JSON string as a liquid_core::Value::scalar
        Ok(Value::scalar(json_string))
    }
}
