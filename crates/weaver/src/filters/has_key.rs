use liquid::model::KString;
use liquid_core::{
    Display_filter, Expression, Filter, FilterParameters, FilterReflection, FromFilterParameters,
    ParseFilter,
};
use liquid_core::{Result, Runtime};
use liquid_core::{Value, ValueView};

#[derive(Clone, ParseFilter, FilterReflection)]
#[filter(
    name = "hasKey",
    description = "return Default, a boolean indicating the existince of a key (regardless of value) in an object.",
    parameters(HasKeyArgs),
    parsed(HasKeyFilter)
)]
pub struct HasKey;

#[derive(Debug, FilterParameters)]
struct HasKeyArgs {
    #[parameter(description = "The key to check for.", arg_type = "str")]
    key: Expression,
}

#[derive(Debug, FromFilterParameters, Display_filter)]
#[name = "hasKey"]
struct HasKeyFilter {
    #[parameters]
    args: HasKeyArgs,
}

impl Filter for HasKeyFilter {
    fn evaluate(&self, input: &dyn ValueView, runtime: &dyn Runtime) -> Result<Value> {
        let serde_value = input.to_value();

        let args = self.args.evaluate(runtime)?;
        let key_exists = match serde_value {
            Value::Object(map) => map.contains_key(&KString::from(args.key)),
            _ => false,
        };

        Ok(Value::scalar(key_exists))
    }
}
