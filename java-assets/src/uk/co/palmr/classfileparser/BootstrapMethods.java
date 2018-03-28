package uk.co.palmr.classfileparser;

import java.util.function.Supplier;

public class BootstrapMethods {
    public static void main(String[] args) {
        takesLambda(() -> "Hello World");
    }

    private static void takesLambda(Supplier<String> stringSupplier) {
        System.out.println(stringSupplier.get());
    }
}
