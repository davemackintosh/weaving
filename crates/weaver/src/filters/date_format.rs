use chrono::{DateTime, Utc};
use liquid_core::{
    Display_filter, Expression, Filter, FilterParameters, FilterReflection, FromFilterParameters,
    ParseFilter,
};
use liquid_core::{Result, Runtime};
use liquid_core::{Value, ValueView};

#[derive(Clone, ParseFilter, FilterReflection)]
#[filter(
    name = "date",
    description = "Format a date.",
    parameters(DateArgs),
    parsed(DateFilter)
)]
pub struct Date;

#[derive(Debug, FilterParameters)]
struct DateArgs {
    #[parameter(description = "The format.", arg_type = "str")]
    format: Expression,
}

#[derive(Debug, FromFilterParameters, Display_filter)]
#[name = "hasKey"]
struct DateFilter {
    #[parameters]
    args: DateArgs,
}

impl Filter for DateFilter {
    fn evaluate(&self, input: &dyn ValueView, runtime: &dyn Runtime) -> Result<Value> {
        let args = self.args.evaluate(runtime)?;
        let date: DateTime<Utc> = input.to_kstr().parse().unwrap();

        let formatted = format!("{}", date.format(&args.format));

        Ok(Value::scalar(formatted))
    }
}
