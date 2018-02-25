package uk.co.palmr.classfileparser;

import java.util.ArrayList;
import java.util.List;

public class AllocateClasses {
  public static void main(String[] args) {
    {
      List<BasicClass> instances = new ArrayList<>(100);

      for (int i = 0; i < 100; i++) {
        BasicClass lNewObject = new BasicClass("Instance", i);
        instances.add(lNewObject);
      }

      System.out.println("Objects should now be allocated");

      // Remove references to those instances
      instances.clear();
    }

    // Attempt a GC?
    System.gc();

    System.out.println("Objects should now be gone?");
  }
}
