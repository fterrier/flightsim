#include "engine.h"
#include <stdio.h>

void proj2_hello() {
  printf("hello, world\n");
}

int main(int argc, char *argv[]) {
  Bla *bla = new Bla();
  bla->bla();

  proj2_hello();

  return 0;
}

void test() {
  Bla *bla2 = new Bla();
}

