package uk.co.palmr.classfileparser;

public class SimpleMath {
  public static void main(String[] args) {
    System.out.println("Integer math:");
    intMath();

    System.out.println("Float math:");
    floatMath();
  }

  private static void intMath() {
    // Create two ints
    int a = 10;
    int b = 20;

    // Sum int a and int b into int c
    int c = a + b;

    // Output the value of c
    System.out.println(c);
  }

  private static void floatMath() {
    // Create two floats
    float a = 1.0f;
    float b = 3.0f;

    // Sum float a and float b into float c
    float c = a / b;

    // Output the value of c
    System.out.println(c);
  }
}
