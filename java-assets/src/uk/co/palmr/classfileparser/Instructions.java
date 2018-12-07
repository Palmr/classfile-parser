package uk.co.palmr.classfileparser;

public class Instructions {
  public static int[] test(int x) {
    int a = 0;
    switch (x) {
      case 1: a = 10; break;
      case 2: a = 20; break;
      case 3: a = 30; break;
    }
    int b = 0;
    switch (x) {
      case 100: b = 2; break;
      case 1000: b = 3; break;
      case 10000: b = 4; break;
    }
    System.out.printf("%d + %d = %d\n", a, b, a + b);
    return new int[]{ a, b, a + b };
  }
}
