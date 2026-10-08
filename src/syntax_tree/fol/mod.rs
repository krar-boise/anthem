use anyhow;

pub mod sigma_0;

pub(crate) trait IntegerConversion {
    fn convert_to_integer_domain(self) -> anyhow::Result<Self>
    where
        Self: Sized;
}
