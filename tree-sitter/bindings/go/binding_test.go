package tree_sitter_nutrition_test

import (
	"testing"

	tree_sitter "github.com/tree-sitter/go-tree-sitter"
	tree_sitter_nutrition "github.com/jafayer/nutrition-rs/bindings/go"
)

func TestCanLoadGrammar(t *testing.T) {
	language := tree_sitter.NewLanguage(tree_sitter_nutrition.Language())
	if language == nil {
		t.Errorf("Error loading Nutrition grammar")
	}
}
