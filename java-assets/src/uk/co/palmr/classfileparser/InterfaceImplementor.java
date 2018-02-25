package uk.co.palmr.classfileparser;

public class InterfaceImplementor implements BasicInterface {

  public static void main(String[] args) {
    InterfaceImplementor object = new InterfaceImplementor();
    System.out.println(object.getOutputMessage());
  }

  @Override
  public String getOutputMessage() {
    return "Output message from interface impl";
  }
}
