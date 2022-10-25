use crate::{
    argument::CommandArgument,
    builder::{FrameworkBuilder, WrappedClient},
    command::{Command, CommandMap},
    context::{AutocompleteContext, Focused, SlashContext},
    group::{GroupParent, ParentGroupMap, ParentType},
    hook::{AfterHook, BeforeHook},
    twilight_exports::{
        ApplicationMarker, Client,
        Command as TwilightCommand, CommandData, CommandDataOption, CommandOption, CommandOptionType,
        CommandOptionValue, GuildMarker, Id, Interaction, InteractionData, InteractionType, InteractionClient, InteractionResponse,
        InteractionResponseType, OptionsCommandOptionData,
    },
    waiter::WaiterWaker
};
use tracing::debug;
use parking_lot::Mutex;

macro_rules! extract {
    ($expr:expr => $variant:ident) => {
        match $expr {
            InteractionData::$variant(inner) => inner,
            _ => unreachable!()
        }
    };
}

macro_rules! focused {
    ($($tt:tt)*) => {
        match $($tt)* {
            CommandOptionValue::Focused(input, kind) => Focused {
                input: input.clone(),
                kind: *kind
            },
            _ => return None
        }
    };
}

/// The framework used to dispatch slash commands.
pub struct Framework<D> {
    /// The http client used by the framework.
    pub http_client: WrappedClient,
    /// The application id of the client.
    pub application_id: Id<ApplicationMarker>,
    /// Data shared across all command and hook invocations.
    pub data: D,
    /// A map of simple commands.
    pub commands: CommandMap<D>,
    /// A map of command groups including all children.
    pub groups: ParentGroupMap<D>,
    /// A hook executed before the command.
    pub before: Option<BeforeHook<D>>,
    /// A hook executed after command's execution.
    pub after: Option<AfterHook<D>>,
    pub waiters: Mutex<Vec<WaiterWaker<D>>>
}

impl<D> Framework<D> {
    /// Creates a new [Framework](self::Framework) from the given builder.
    pub(crate) fn from_builder(builder: FrameworkBuilder<D>) -> Self {
        Self {
            http_client: builder.http_client,
            application_id: builder.application_id,
            data: builder.data,
            commands: builder.commands,
            groups: builder.groups,
            before: builder.before,
            after: builder.after,
            waiters: Mutex::new(Vec::new())
        }
    }

    /// Creates a new framework builder, this is a shortcut to FrameworkBuilder.
    /// [new](crate::builder::FrameworkBuilder::new)
    pub fn builder(
        http_client: impl Into<WrappedClient>,
        application_id: Id<ApplicationMarker>,
        data: D,
    ) -> FrameworkBuilder<D> {
        FrameworkBuilder::new(http_client, application_id, data)
    }

    /// Gets the http client used by the framework.
    pub fn http_client(&self) -> &Client {
        self.http_client.inner()
    }

    /// Gets the [interaction client](InteractionClient) using this framework's
    /// [http client](Client) and [application id](ApplicationMarker)
    pub fn interaction_client(&self) -> InteractionClient {
        self.http_client().interaction(self.application_id)
    }

    /// Processes the given interaction, dispatching commands or waking waiters if necessary.
    pub async fn process(&self, interaction: Interaction) {
        match interaction.kind {
            InteractionType::ApplicationCommand => self.try_execute(interaction).await,
            InteractionType::ApplicationCommandAutocomplete => self.try_autocomplete(interaction).await,
            InteractionType::MessageComponent => {
                let mut lock = self.waiters.lock();
                if let Some(position) = lock.iter().position(|waker| waker.check(self, &interaction)) {
                    lock.remove(position).wake(interaction);
                }
            }
            _ => ()
        }
    }

    /// Tries to execute a command based on the given
    /// [ApplicationCommand](ApplicationCommand).
    async fn try_execute(&self, mut interaction: Interaction) {
        if let Some(command) = self.get_command(&mut interaction) {
            self.execute(command, interaction).await;
        }
    }

    async fn try_autocomplete(&self, mut interaction: Interaction) {
        if let Some((argument, value)) = self.get_autocomplete_argument(extract!(interaction.data.as_ref().unwrap() => ApplicationCommand)) {
            if let Some(fun) = &argument.autocomplete {
                let context = AutocompleteContext::new(
                    &self.http_client,
                    &self.data,
                    value,
                    &mut interaction,
                );
                let data = (fun.0)(context).await;

                let _ = self
                    .interaction_client()
                    .create_response(
                        interaction.id,
                        &interaction.token,
                        &InteractionResponse {
                            kind: InteractionResponseType::ApplicationCommandAutocompleteResult,
                            data,
                        },
                    )
                    .exec()
                    .await;
            }
        }
    }

    fn get_autocomplete_argument(
        &self,
        data: &CommandData,
    ) -> Option<(&CommandArgument<D>, Focused)> {
        if !data.options.is_empty() {
            let outer = data.options.get(0)?;
            match &outer.value {
                CommandOptionValue::SubCommandGroup(sc_group) => {
                    if !sc_group.is_empty() {
                        let map = self
                            .groups
                            .get(data.name.as_str())?
                            .kind
                            .as_group()?;
                        let group = map.get(outer.name.as_str())?;
                        let next = sc_group.get(0)?;
                        if let CommandOptionValue::SubCommand(options) = &next.value {
                            let focused = self.get_focus(options)?;
                            let command = group.subcommands.get(next.name.as_str())?;
                            let position = command
                                .arguments
                                .iter()
                                .position(|arg| arg.name == focused.name)?;
                            return Some((command.arguments.get(position)?, focused!(&focused.value)));
                        }
                    }
                }
                CommandOptionValue::SubCommand(sc) => {
                    if !sc.is_empty() {
                        let group = self.groups.get(data.name.as_str())?
                            .kind
                            .as_simple()?;
                        let focused = self.get_focus(sc)?;
                        let command = group.get(outer.name.as_str())?;
                        let position = command
                            .arguments
                            .iter()
                            .position(|arg| arg.name == focused.name)?;
                        return Some((command.arguments.get(position)?, focused!(&focused.value)));
                    }
                }
                _ => {
                    let focused = self.get_focus(&data.options)?;
                    let command = self.commands.get(data.name.as_str())?;
                    let position = command
                        .arguments
                        .iter()
                        .position(|arg| arg.name == focused.name)?;
                    return Some((command.arguments.get(position)?, focused!(&focused.value)));
                }
            }
        }

        None
    }

    fn get_focus<'a>(&self, data: &'a Vec<CommandDataOption>) -> Option<&'a CommandDataOption> {
        for item in data {
            if let CommandOptionValue::Focused(..) = &item.value {
                return Some(item);
            }
        }
        None
    }

    /// Gets the command matching the given
    /// [ApplicationCommand](ApplicationCommand),
    /// returning `None` if no command matches the given interaction.
    fn get_command(&self, interaction: &mut Interaction) -> Option<&Command<D>> {
        let data = interaction.data.as_mut()?;
        let interaction_data = extract!(data => ApplicationCommand);
        if let Some(next) = self.get_next(&mut interaction_data.options) {
            let group = self.groups.get(&*interaction_data.name)?;
            match next.value.kind() {
                CommandOptionType::SubCommand => {
                    let subcommands = group.kind.as_simple()?;
                    let options = match next.value {
                        CommandOptionValue::SubCommand(s) => s,
                        _ => unreachable!(),
                    };
                    interaction_data.options = options;
                    subcommands.get(&*next.name)
                }
                CommandOptionType::SubCommandGroup => {
                    let mut options = match next.value {
                        CommandOptionValue::SubCommandGroup(s) => s,
                        _ => unreachable!(),
                    };
                    let subcommand = self.get_next(&mut options)?;
                    let subgroups = group.kind.as_group()?;
                    let group = subgroups.get(&*next.name)?;
                    let options = match subcommand.value {
                        CommandOptionValue::SubCommand(s) => s,
                        _ => unreachable!(),
                    };
                    interaction_data.options = options;
                    group.subcommands.get(&*subcommand.name)
                }
                _ => None,
            }
        } else {
            self.commands.get(&*interaction_data.name)
        }
    }

    /// Gets the next [option](CommandDataOption)
    /// only if it corresponds to a subcommand or a subcommand group.
    fn get_next(&self, interaction: &mut Vec<CommandDataOption>) -> Option<CommandDataOption> {
        if !interaction.is_empty()
            && (interaction[0].value.kind() == CommandOptionType::SubCommand
                || interaction[0].value.kind() == CommandOptionType::SubCommandGroup)
        {
            Some(interaction.remove(0))
        } else {
            None
        }
    }

    /// Executes the given [command](crate::command::Command) and the hooks.
    async fn execute(&self, cmd: &Command<D>, interaction: Interaction) {
        let context = SlashContext::new(
            &self.http_client,
            self.application_id,
            &self.data,
            interaction,
        );

        let execute = if let Some(before) = &self.before {
            (before.0)(&context, cmd.name).await
        } else {
            true
        };

        if execute {
            let result = (cmd.fun)(&context).await;

            if let Some(after) = &self.after {
                (after.0)(&context, cmd.name, result).await;
            }
        }
    }

    /// Registers the commands provided to the framework in the specified guild.
    pub async fn register_guild_commands(
        &self,
        guild_id: Id<GuildMarker>,
    ) -> Result<Vec<TwilightCommand>, Box<dyn std::error::Error + Send + Sync>> {
        let mut commands = Vec::new();

        for cmd in self.commands.values() {
            let mut options = Vec::new();

            for i in &cmd.arguments {
                options.push(i.as_option());
            }
            let interaction_client = self.interaction_client();
            let mut command = interaction_client
                .create_guild_command(guild_id)
                .chat_input(cmd.name, cmd.description)?
                .command_options(&options)?;

            if let Some(permissions) = &cmd.required_permissions {
                command = command.default_member_permissions(*permissions);
            }

            commands.push(command.exec().await?.model().await?);
        }

        for group in self.groups.values() {
            let options = self.create_group(group);
            let interaction_client = self.interaction_client();
            let mut command = interaction_client
                .create_guild_command(guild_id)
                .chat_input(group.name, group.description)?
                .command_options(&options)?;

            if let Some(permissions) = &group.required_permissions {
                command = command.default_member_permissions(*permissions);
            }

            commands.push(command.exec().await?.model().await?);
        }

        Ok(commands)
    }

    /// Registers the commands provided to the framework globally.
    pub async fn register_global_commands(
        &self,
    ) -> Result<Vec<TwilightCommand>, Box<dyn std::error::Error + Send + Sync>> {
        let mut commands = Vec::new();

        for cmd in self.commands.values() {
            let mut options = Vec::new();

            for i in &cmd.arguments {
                options.push(i.as_option());
            }
            let interaction_client = self.interaction_client();
            let mut command = interaction_client
                .create_global_command()
                .chat_input(cmd.name, cmd.description)?
                .command_options(&options)?;

            if let Some(permissions) = &cmd.required_permissions {
                command = command.default_member_permissions(*permissions);
            }

            commands.push(command.exec().await?.model().await?);
        }

        for group in self.groups.values() {
            let options = self.create_group(group);
            let interaction_client = self.interaction_client();
            let mut command = interaction_client
                .create_global_command()
                .chat_input(group.name, group.description)?
                .command_options(&options)?;

            if let Some(permissions) = &group.required_permissions {
                command = command.default_member_permissions(*permissions);
            }

            commands.push(command.exec().await?.model().await?);
        }

        Ok(commands)
    }

    fn arg_options(&self, arguments: &Vec<CommandArgument<D>>) -> Vec<CommandOption> {
        let mut options = Vec::with_capacity(arguments.len());

        for arg in arguments {
            options.push(arg.as_option());
        }

        options
    }

    fn create_group(&self, parent: &GroupParent<D>) -> Vec<CommandOption> {
        debug!("Registering group {}", parent.name);

        if let ParentType::Group(map) = &parent.kind {
            let mut subgroups = Vec::new();
            for group in map.values() {
                debug!("Registering subgroup {} of {}", group.name, parent.name);

                let mut subcommands = Vec::new();
                for sub in group.subcommands.values() {
                    subcommands.push(self.create_subcommand(sub))
                }

                subgroups.push(CommandOption::SubCommandGroup(OptionsCommandOptionData {
                    name: group.name.to_string(),
                    description: group.description.to_string(),
                    options: subcommands,
                    ..Default::default()
                }));
            }
            subgroups
        } else if let ParentType::Simple(map) = &parent.kind {
            let mut subcommands = Vec::new();
            for sub in map.values() {
                subcommands.push(self.create_subcommand(sub));
            }

            subcommands
        } else {
            unreachable!()
        }
    }

    /// Creates a subcommand at the given scope.
    fn create_subcommand(&self, cmd: &Command<D>) -> CommandOption {
        debug!("Registering {} subcommand", cmd.name);

        CommandOption::SubCommand(OptionsCommandOptionData {
            name: cmd.name.to_string(),
            description: cmd.description.to_string(),
            options: self.arg_options(&cmd.arguments),
            ..Default::default()
        })
    }
}
