@Deprecated
public class DeprecatedAnnotation {
  @Deprecated
  private String deprecatedField;

  @Deprecated(since = "1.2.3", forRemoval = true)
  public void deprecatedMethod() {
    System.out.println("Don't call me");
  }
}
