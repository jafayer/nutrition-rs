use crate::ast::ast::Ingredient;
use crate::emitters::emitter::{CanEmit, format_quantity, quoted_string};

#[cfg(feature = "runtime")]
use crate::emitters::emitter::CanEmitAI;
#[cfg(feature = "runtime")]
use async_openai::{
    Client,
    types::chat::{
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage,
        ChatCompletionRequestUserMessage, CreateChatCompletionRequestArgs,
    },
};
#[cfg(feature = "runtime")]
use async_trait::async_trait;

#[cfg(feature = "runtime")]
use schemars::schema_for;

pub struct IngredientEmitter;

const INGREDIENT_KEYWORD: &str = "@ingredient";

impl CanEmit<Ingredient> for IngredientEmitter {
    fn emit(&self, ingredient: &Ingredient) -> String {
        let mut output = String::new();

        // Emit ingredient keyword
        output.push_str(INGREDIENT_KEYWORD);

        // Emit quantity
        for quantity in &ingredient.quantities {
            output.push('(');
            output.push_str(&format_quantity(quantity));
            output.push(')');
        }
        output.push(' ');

        // Emit label
        for alias in &ingredient.aliases {
            quoted_string(&mut output, alias);
            output.push(' ');
        }

        // Emit properties if any
        if !ingredient.properties.is_empty() {
            output.push('{');
            for property in &ingredient.properties {
                output.push('\n');
                output.push_str("    "); // indent
                output.push_str(&property.name); // property names are NOT quoted
                output.push_str(": ");
                output.push_str(&format_quantity(&property.value));
            }
            output.push('\n');
            output.push('}'); // close block
        } else {
            // add empty properties list
            output.push_str(" { }");
        }

        output.push('\n');

        output
    }
}

#[cfg(feature = "runtime")]
#[async_trait]
impl CanEmitAI<Ingredient> for IngredientEmitter {
    async fn emit_ai(&self, ingredient: &Ingredient) -> Result<Ingredient, String> {
        let client = Client::new();
        let schema = schema_for!(Ingredient);
        let schema_json = serde_json::to_string_pretty(&schema)
            .map_err(|e| format!("Failed to serialize schema: {}", e))?;

        let request = CreateChatCompletionRequestArgs::default()
            .model("gpt-4.1-mini")
            .max_tokens(1024u32)
            .messages(vec![
                ChatCompletionRequestMessage::System(ChatCompletionRequestSystemMessage::from(format!(
                    "You are an expert nutrition data formatter. Return accurate nutrition data for the following ingredient, in the correct format according to the following schema. Be sure to include all relevant nutritional information, including calories, macronutrients, and micronutrients. Also include serving quantities in one or more units depending on what makes sense for the particular food:\n\n{}",
                    schema_json
                ))),
                ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessage::from(format!(
                    "Generate the nutrition data for the following ingredient: {:?}",
                    ingredient
                ))),
            ])
            .build().map_err(|e| e.to_string())?;

        let response = client
            .chat()
            .create(request)
            .await
            .map_err(|e| format!("OpenAI API request failed: {}", e))?;

        match response.choices.first() {
            Some(choice) => match &choice.message.content {
                Some(content) => serde_json::from_str(content)
                    .map_err(|e| format!("Failed to parse AI response: {}", e)),
                None => Err("No content in OpenAI API response".to_string()),
            },
            None => Err("No response from OpenAI API".to_string()),
        }
    }
}
