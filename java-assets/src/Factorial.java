public class Factorial {
    public static int factorial(int i)
    {
        return i < 1 ? 1 : i * factorial(i - 1);
    }
};

