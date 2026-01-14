#[derive(Debug)]
pub struct Document {
    pub items: Vec<Item>,
}

#[derive(Debug)]
pub enum Item {
    Property(Property),
    Ingredient(Ingredient),
    // Food(Food),
    Recipe(Recipe),
    Exercise(Exercise),
    Day(Day),
    // Meal(Meal),
    Ate(Ate),
    Exercised(Exercised),
    Comment(String),
}

#[derive(Debug)]
pub struct Quantity {
    pub amount: f64,
    pub unit: Option<String>,
}

#[derive(Debug)]
pub struct Property {
    pub name: String,
    pub value: Quantity,
}

#[derive(Debug)]
pub struct Ingredient {
    pub aliases: Vec<String>,
    pub quantities: Vec<Quantity>,
    pub properties: Vec<Property>,
}

#[derive(Debug)]
pub struct IngredientLabel {
    pub alias: String,
    pub quantity: Quantity,
}

#[derive(Debug)]
pub struct Recipe {
    pub aliases: Vec<String>,
    pub quantities: Vec<Quantity>,
    pub ingredients: Vec<IngredientLabel>,
}

#[derive(Debug)]
pub struct Exercise {
    pub aliases: Vec<String>,
    pub quantities: Vec<Quantity>,
    pub properties: Vec<Property>,
}

#[derive(Debug)]
pub struct Ate {
    pub food_alias: String,
    pub quantity: Quantity,
}

#[derive(Debug)]
pub struct Exercised {
    pub exercise_alias: String,
    pub quantity: Quantity,
}

#[derive(Debug)]
pub enum DayItem {
    Ate(Ate),
    Exercised(Exercised),
}

#[derive(Debug)]
pub struct Day {
    pub date: String,
    pub items: Vec<DayItem>,
}