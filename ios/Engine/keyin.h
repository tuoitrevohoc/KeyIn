// C interface of the Rust engine in src/lib.rs.
#pragma once
char *keyin_compose_context(const char *raw, unsigned int mode, unsigned int previous);
char *keyin_correct(const char *raw, unsigned int mode);
unsigned int keyin_language_options(const char *raw);
char *keyin_analyze(const char *raw, unsigned int mode, unsigned int previous, unsigned int correct_mode);
unsigned int keyin_next_letters(const char *raw);
void keyin_free(char *ptr);
