#include <stdio.h>
#include <unistd.h>
int main(void) {
    setvbuf(stdout, NULL, _IOLBF, 0);
    for (;;) { puts("Hello from TrpHot. Edit main.c and save!"); sleep(2); }
}
