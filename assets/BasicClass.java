package uk.co.palmr.karl.examples;

public class BasicClass {
  private final String mString;
  private final Integer mInteger;

  public BasicClass(String pString, Integer pInteger) {
    mString = pString;
    mInteger = pInteger;
  }

  public String getString() {
    return mString;
  }

  public Integer getInteger() {
    return mInteger;
  }
}
