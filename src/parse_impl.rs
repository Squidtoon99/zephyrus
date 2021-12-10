use crate::prelude::*;
use crate::twilight_exports::*;

#[async_trait]
impl<T: Send + Sync + 'static> Parse<T> for String {
    async fn parse(
        _: &Client,
        _: &T,
        value: Option<&CommandOptionValue>,
    ) -> Result<Self, ParseError> {
        if let Some(kind) = value {
            if let CommandOptionValue::String(s) = kind {
                return Ok(s.to_owned());
            }
        }
        Err("String expected".into())
    }

    fn option_type() -> CommandOptionType {
        CommandOptionType::String
    }
}

#[async_trait]
impl<T: Send + Sync + 'static> Parse<T> for i64 {
    async fn parse(
        _: &Client,
        _: &T,
        value: Option<&CommandOptionValue>,
    ) -> Result<Self, ParseError> {
        if let Some(kind) = value {
            if let CommandOptionValue::Integer(i) = kind {
                return Ok(*i);
            }
        }
        Err("Integer expected".into())
    }

    fn option_type() -> CommandOptionType {
        CommandOptionType::Integer
    }
}

#[async_trait]
impl<T: Send + Sync + 'static> Parse<T> for u64 {
    async fn parse(
        _: &Client,
        _: &T,
        value: Option<&CommandOptionValue>,
    ) -> Result<Self, ParseError> {
        if let Some(kind) = value {
            if let CommandOptionValue::Integer(i) = kind {
                return Ok(*i as u64);
            }
        }
        Err("Integer expected".into())
    }

    fn option_type() -> CommandOptionType {
        CommandOptionType::Integer
    }
}

#[async_trait]
impl<T: Send + Sync + 'static> Parse<T> for f64 {
    async fn parse(
        _: &Client,
        _: &T,
        value: Option<&CommandOptionValue>,
    ) -> Result<Self, ParseError> {
        if let Some(kind) = value {
            if let CommandOptionValue::Number(i) = kind {
                return Ok(i.0);
            }
        }
        Err("Number expected".into())
    }

    fn option_type() -> CommandOptionType {
        CommandOptionType::Number
    }
}

#[async_trait]
impl<T: Send + Sync + 'static> Parse<T> for bool {
    async fn parse(
        _: &Client,
        _: &T,
        value: Option<&CommandOptionValue>,
    ) -> Result<Self, ParseError> {
        if let Some(kind) = value {
            if let CommandOptionValue::Boolean(i) = kind {
                return Ok(*i);
            }
        }
        Err("Boolean expected".into())
    }

    fn option_type() -> CommandOptionType {
        CommandOptionType::Boolean
    }
}

#[async_trait]
impl<T: Send + Sync + 'static> Parse<T> for ChannelId {
    async fn parse(
        _: &Client,
        _: &T,
        value: Option<&CommandOptionValue>,
    ) -> Result<Self, ParseError> {
        if let Some(kind) = value {
            if let CommandOptionValue::Channel(channel) = kind {
                return Ok(*channel);
            }
        }

        Err("Channel expected".into())
    }

    fn option_type() -> CommandOptionType {
        CommandOptionType::Channel
    }
}

#[async_trait]
impl<T: Send + Sync + 'static> Parse<T> for UserId {
    async fn parse(
        _: &Client,
        _: &T,
        value: Option<&CommandOptionValue>,
    ) -> Result<Self, ParseError> {
        if let Some(kind) = value {
            if let CommandOptionValue::User(user) = kind {
                return Ok(*user);
            }
        }

        Err("User expected".into())
    }

    fn option_type() -> CommandOptionType {
        CommandOptionType::User
    }
}

#[async_trait]
impl<T: Send + Sync + 'static> Parse<T> for RoleId {
    async fn parse(
        _: &Client,
        _: &T,
        value: Option<&CommandOptionValue>,
    ) -> Result<Self, ParseError> {
        if let Some(kind) = value {
            if let CommandOptionValue::Role(role) = kind {
                return Ok(*role);
            }
        }

        Err("Role expected".into())
    }

    fn option_type() -> CommandOptionType {
        CommandOptionType::Role
    }
}

#[async_trait]
impl<T: Send + Sync + 'static> Parse<T> for GenericId {
    async fn parse(
        _: &Client,
        _: &T,
        value: Option<&CommandOptionValue>,
    ) -> Result<Self, ParseError> {
        if let Some(kind) = value {
            if let CommandOptionValue::Mentionable(id) = kind {
                return Ok(*id);
            }
        }

        Err("Mentionable expected".into())
    }

    fn option_type() -> CommandOptionType {
        CommandOptionType::Mentionable
    }
}

#[async_trait]
impl<T: Parse<E>, E: Send + Sync + 'static> Parse<E> for Option<T> {
    async fn parse(
        http_client: &Client,
        data: &E,
        value: Option<&CommandOptionValue>,
    ) -> Result<Self, ParseError> {
        if let Ok(parsed) = T::parse(http_client, data, value).await {
            Ok(Some(parsed))
        } else {
            Ok(None)
        }
    }

    fn is_required() -> bool {
        false
    }

    fn option_type() -> CommandOptionType {
        T::option_type()
    }
}

#[async_trait]
impl<T, E, C> Parse<C> for Result<T, E>
where
    T: Parse<C>,
    E: From<ParseError>,
    C: Send + Sync + 'static,
{
    async fn parse(
        http_client: &Client,
        data: &C,
        value: Option<&CommandOptionValue>,
    ) -> Result<Self, ParseError> {
        // as we want to return the error if occurs, we'll map the error and always return Ok
        Ok(T::parse(http_client, data, value).await.map_err(From::from))
    }

    fn is_required() -> bool {
        T::is_required()
    }

    fn option_type() -> CommandOptionType {
        T::option_type()
    }
}

macro_rules! impl_derived_parse {
    ($([$($derived:ty),+] from $prim:ty),* $(,)?) => {
        $($(
            #[async_trait]
            impl<T: Send + Sync + 'static> Parse<T> for $derived {
                async fn parse(
                    http_client: &Client,
                    data: &T,
                    value: Option<&CommandOptionValue>
                ) -> Result<Self, ParseError> {
                    let p = <$prim>::parse(http_client, data, value).await?;

                    if p > <$derived>::MAX as $prim {
                        Err(
                            concat!(
                                "Failed to parse to ",
                                stringify!($derived),
                                ": the value is greater than ",
                                stringify!($derived),
                                "'s ",
                                "range of values"
                            ).into()
                        )
                    } else if p < <$derived>::MIN as $prim {
                        Err(
                            concat!(
                                "Failed to parse to ",
                                stringify!($derived),
                                ": the value is less than ",
                                stringify!($derived),
                                "'s ",
                                "range of values"
                            ).into()
                        )
                    } else {
                        Ok(p as $derived)
                    }
                }

                fn option_type() -> CommandOptionType {
                    <$prim as Parse<T>>::option_type()
                }
            }
        )*)*
    };
}

impl_derived_parse! {
    [i8, i16, i32, i128, isize] from i64,
    [u8, u16, u32, u128, usize] from u64,
    [f32] from f64,
}