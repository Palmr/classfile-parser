public class BasicClass {
  private final String stringField;
  private final Integer integerField;

  BasicClass(final String string, final Integer integer) {
    stringField = string;
    integerField = integer;
  }

  public String getStringField() {
    return stringField;
  }

  public Integer getIntegerField() {
    return integerField;
  }

  public static String getName() {
    return "Hallo!";
  }

  public static long getSize() {
    return 1234567L;
  }

  public static double getLEETness() {
    return 1.337;
  }

  public static int add(int a, int b) {
      return a + b;
  }
}
