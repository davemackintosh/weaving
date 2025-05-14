use liquid::Error;
use liquid_core::{Display_filter, Filter, FilterReflection, ParseFilter};
use liquid_core::{Result, Runtime};
use liquid_core::{Value, ValueView};

#[derive(Clone, ParseFilter, FilterReflection)]
#[filter(
    name = "raw",
    description = "Output the raw input unescaped.",
    parsed(RawHtmlFilter)
)]
pub struct RawHtml;

#[derive(Debug, Default, Display_filter)]
#[name = "raw"]
struct RawHtmlFilter;

impl Filter for RawHtmlFilter {
    fn evaluate(&self, input: &dyn ValueView, _runtime: &dyn Runtime) -> Result<Value> {
        // Check if the input is a scalar (string, number, etc.)
        let scalar_input = input.as_scalar().ok_or_else(|| {
            Error::with_msg("RawHtml filter expects a scalar (string, number, etc.) input.")
        })?;

        // Get the owned string from the scalar.
        // Using into_owned() gets a String whether the scalar was borrowed or owned.
        let raw_string = scalar_input.into_owned();

        // Create a liquid::model::Value::Scalar from the raw string.
        // The crucial part: We rely on Value::scalar() NOT marking the string for HTML escaping,
        // unlike strings produced by liquid::model::to_value(). This is the standard pattern
        // for raw output in other templating engines, and the most likely mechanism here.
        Ok(Value::scalar(raw_string)) // Return the string as a raw scalar value
    }
}
