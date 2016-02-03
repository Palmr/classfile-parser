package uk.co.palmr.karl.examples;

import java.util.ArrayList;
import java.util.List;

public class AllocateClasses {
  public static void main(String[] args) {
    {
      List<BasicClass> mInstanceList = new ArrayList<>(100);

      for (int i = 0; i < 100; i++) {
        BasicClass lNewObject = new BasicClass("Instance", i);
        mInstanceList.add(lNewObject);
      }

      System.out.println("Objects should now be allocated");

      mInstanceList = null;
    }

    // Attempt a GC?
    System.gc();

    System.out.println("Objects should now be gone?");
  }
}
