use crate::app::AppAction;
use crate::context::AppContext;
use crate::model::qualified_identity::QualifiedIdentity;
use crate::ui::components::top_panel::add_top_panel;
use crate::ui::ScreenLike;
use dash_sdk::dpp::identity::accessors::IdentityGettersV0;
use dash_sdk::dpp::identity::hash::IdentityPublicKeyHashMethodsV0;
use dash_sdk::dpp::identity::identity_public_key::accessors::v0::IdentityPublicKeyGettersV0;
use dash_sdk::dpp::identity::Identity;
use dash_sdk::dpp::prelude::IdentityPublicKey;
use eframe::egui::{self, Context};
use egui::{RichText, TextEdit};
use std::sync::Arc;

pub struct KeyInfoScreen {
    pub identity: QualifiedIdentity,
    pub key: IdentityPublicKey,
    pub private_key_bytes: Option<Vec<u8>>,
    pub app_context: Arc<AppContext>,
    private_key_input: String,
    error_message: Option<String>,
}

impl ScreenLike for KeyInfoScreen {
    fn refresh(&mut self) {}

    fn ui(&mut self, ctx: &Context) -> AppAction {
        let action = add_top_panel(
            ctx,
            &self.app_context,
            vec![
                ("Identities", AppAction::GoToMainScreen),
                ("Key Info", AppAction::None),
            ],
            None,
        );

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Key Information");

            egui::Grid::new("key_info_grid")
                .num_columns(2)
                .spacing([10.0, 10.0])
                .striped(true)
                .show(ui, |ui| {
                    // Key ID
                    ui.label(RichText::new("Key ID:").strong());
                    ui.label(format!("{}", self.key.id()));
                    ui.end_row();

                    // Purpose
                    ui.label(RichText::new("Purpose:").strong());
                    ui.label(format!("{:?}", self.key.purpose()));
                    ui.end_row();

                    // Security Level
                    ui.label(RichText::new("Security Level:").strong());
                    ui.label(format!("{:?}", self.key.security_level()));
                    ui.end_row();

                    // Type
                    ui.label(RichText::new("Type:").strong());
                    ui.label(format!("{:?}", self.key.key_type()));
                    ui.end_row();

                    // Read Only
                    ui.label(RichText::new("Read Only:").strong());
                    ui.label(format!("{}", self.key.read_only()));
                    ui.end_row();
                });

            ui.separator();

            // Display the private key if available
            if let Some(private_key) = &self.private_key_bytes {
                ui.label("Private Key:");
                let private_key_hex = hex::encode(private_key);
                ui.add(
                    TextEdit::multiline(&mut private_key_hex.as_str().to_owned())
                        .desired_width(f32::INFINITY),
                );
            } else {
                ui.label("Enter Private Key:");
                ui.text_edit_singleline(&mut self.private_key_input);

                if ui.button("Add Private Key").clicked() {
                    self.validate_and_store_private_key();
                }

                // Display error message if validation fails
                if let Some(error_message) = &self.error_message {
                    ui.colored_label(egui::Color32::RED, error_message);
                }
            }
        });

        action
    }
}

impl KeyInfoScreen {
    pub fn new(
        identity: QualifiedIdentity,
        key: IdentityPublicKey,
        private_key_bytes: Option<Vec<u8>>,
        app_context: &Arc<AppContext>,
    ) -> Self {
        Self {
            identity,
            key,
            private_key_bytes,
            app_context: app_context.clone(),
            private_key_input: String::new(),
            error_message: None,
        }
    }

    fn validate_and_store_private_key(&mut self) {
        // Convert the input string to bytes (hex decoding)
        match hex::decode(&self.private_key_input) {
            Ok(private_key_bytes) => {
                let validation_result = self
                    .key
                    .validate_private_key_bytes(&private_key_bytes, self.app_context.network);
                if let Err(err) = validation_result {
                    self.error_message = Some(format!("Issue verifying private key {}", err));
                } else {
                    if validation_result.unwrap() {
                        // If valid, store the private key in the context and reset the input field
                        self.private_key_bytes = Some(private_key_bytes.clone());
                        self.identity.encrypted_private_keys.insert(
                            (self.key.purpose().into(), self.key.id()),
                            (self.key.clone(), private_key_bytes),
                        );
                        match self
                            .app_context
                            .insert_local_qualified_identity(&self.identity)
                        {
                            Ok(_) => {
                                self.error_message = None;
                            }
                            Err(e) => {
                                self.error_message = Some(format!("Issue saving: {}", e));
                            }
                        }
                    } else {
                        self.error_message =
                            Some("Private key does not match the public key.".to_string());
                    }
                }
            }
            Err(_) => {
                self.error_message = Some("Invalid hex string for private key.".to_string());
            }
        }
    }
}
