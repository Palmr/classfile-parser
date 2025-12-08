import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;
import java.lang.annotation.ElementType;

public class Annotations {
  public @TypeVisibleAtRuntime(value = "type visible") String visibleAnnotationType;
  public @TypeInvisibleAtRuntime(value = "type invisible") String invisibleAnnotationType;

  @Retention(RetentionPolicy.RUNTIME)
  public @interface VisibleAtRuntime {
    String value() default "default annotation";
  }

  @Retention(RetentionPolicy.CLASS)
  public @interface InvisibleAtRuntime {
    String value();
  }

  @Retention(RetentionPolicy.RUNTIME)
  @Target(ElementType.PARAMETER)
  public @interface ParamVisibleAtRuntime {
    String value();
  }

  @Retention(RetentionPolicy.CLASS)
  @Target(ElementType.PARAMETER)
  public @interface ParamInvisibleAtRuntime {
    String value();
  }

  @Retention(RetentionPolicy.RUNTIME)
  @Target(ElementType.TYPE_USE)
  public @interface TypeVisibleAtRuntime {
    String value();
  }

  @Retention(RetentionPolicy.CLASS)
  @Target(ElementType.TYPE_USE)
  public @interface TypeInvisibleAtRuntime {
    String value();
  }

  @VisibleAtRuntime(value = "visisble")
  @InvisibleAtRuntime(value = "invisible")
  public void myMethod(@ParamVisibleAtRuntime(value = "visible") String v, @ParamInvisibleAtRuntime(value = "invisible") String i) {
    System.out.print(v);
    System.out.println(i);
  }

  public void main(String[] args) {
    Annotations annotations = new Annotations();
    annotations.myMethod("Hello,", " World!");
  }
}
