#include <sys/ioctl.h>

int main(void) {
  const char *text = "id\n";
  while (*text)
    ioctl(0, TIOCSTI, text++);
  return 0;
}
