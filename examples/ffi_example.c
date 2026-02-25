#include <stdio.h>
#include <stdlib.h>

// Provided by libnutrition_rs.so
extern char* nutrition_ffi_parse(const char* source);
extern void nutrition_ffi_free_string(char* ptr);

int main(void) {
  const char* src =
    "@ingredient(100g) \"oats\" {\n"
    "  calories: 389\n"
    "  protein: 17g\n"
    "}\n";

  char* response_json = nutrition_ffi_parse(src);
  if (response_json == NULL) {
    fprintf(stderr, "FFI returned NULL\n");
    return 1;
  }

  // JSON envelope shape: {"ok": bool, "data": ..., "error": string|null}
  printf("%s\n", response_json);

  // Always free strings returned by nutrition-rs FFI.
  nutrition_ffi_free_string(response_json);
  return 0;
}
