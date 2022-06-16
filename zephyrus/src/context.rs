use crate::{
    builder::WrappedClient,
    command::CommandResult,
    message::Message,
    twilight_exports::*,
    waiter::{WaiterReceiver, WaiterSender},
};
use parking_lot::Mutex;

/// Context given to all functions used to autocomplete arguments.
pub struct AutocompleteContext<'a, D> {
    pub http_client: &'a WrappedClient,
    pub data: &'a D,
    pub user_input: Option<String>,
    pub interaction: &'a mut ApplicationCommandAutocomplete,
}

impl<'a, D> AutocompleteContext<'a, D> {
    pub(crate) fn new(
        http_client: &'a WrappedClient,
        data: &'a D,
        user_input: Option<String>,
        interaction: &'a mut ApplicationCommandAutocomplete,
    ) -> Self {
        Self {
            http_client,
            data,
            user_input,
            interaction,
        }
    }

    /// Gets the http client used by the framework.
    pub fn http_client(&self) -> &Client {
        self.http_client.inner()
    }
}

/// Framework context given to all command functions, this struct contains all the necessary
/// items to respond the interaction and access shared data.
pub struct SlashContext<'a, D> {
    /// The http client used by the framework.
    pub http_client: &'a WrappedClient,
    /// The application id provided to the framework.
    pub application_id: Id<ApplicationMarker>,
    /// An [interaction client](InteractionClient) made out of the framework's [http client](Client)
    pub interaction_client: InteractionClient<'a>,
    /// The data shared across the framework.
    pub data: &'a D,
    /// The interaction itself.
    pub interaction: ApplicationCommand,
}

impl<'a, D> Clone for SlashContext<'a, D> {
    fn clone(&self) -> Self {
        SlashContext {
            http_client: &self.http_client,
            application_id: self.application_id,
            interaction_client: self.http_client.inner().interaction(self.application_id),
            data: &self.data,
            interaction: self.interaction.clone(),
        }
    }
}

impl<'a, D> SlashContext<'a, D> {
    /// Creates a new context.
    pub(crate) fn new(
        http_client: &'a WrappedClient,
        application_id: Id<ApplicationMarker>,
        data: &'a D,
        interaction: ApplicationCommand,
    ) -> Self {
        let interaction_client = http_client.inner().interaction(application_id);
        Self {
            http_client,
            application_id,
            interaction_client,
            data,
            interaction,
        }
    }

    /// Gets the http client used by the framework.
    pub fn http_client(&self) -> &Client {
        self.http_client.inner()
    }

    /// Responds to the interaction with an empty message to allow to respond later.
    ///
    /// When this method is used [update_response](Self::update_response) has to be used to edit the response.
    pub async fn acknowledge(&self) -> CommandResult {
        self.interaction_client
            .create_response(
                self.interaction.id,
                &self.interaction.token,
                &InteractionResponse {
                    kind: InteractionResponseType::DeferredChannelMessageWithSource,
                    data: None,
                },
            )
            .exec()
            .await?;

        Ok(())
    }

    /// Updates the sent interaction, this method is a shortcut to twilight's
    /// [update_interaction_original](InteractionClient::update_response)
    /// but http is automatically provided.
    pub async fn update_response<F>(
        &'a self,
        fun: F,
    ) -> Result<Message<'a, D>, Box<dyn std::error::Error + Send + Sync>>
    where
        F: FnOnce(UpdateResponse<'a>) -> UpdateResponse<'a>,
    {
        let update = fun(self
            .interaction_client
            .update_response(&self.interaction.token));
        Ok(update
            .exec()
            .await?
            .model()
            .await
            .map(|msg| Message::new(&self, msg))?)
    }
}
