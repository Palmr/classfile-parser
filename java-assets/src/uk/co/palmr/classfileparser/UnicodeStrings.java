package uk.co.palmr.classfileparser;

public class UnicodeStrings {
  public static void main(String[] args) {
    final String maths = "2H₂ + O₂ ⇌ 2H₂O, R = 4.7 kΩ, ⌀ 200 mm";
    final String runes = "ᚻᛖ ᚳᚹᚫᚦ ᚦᚫᛏ ᚻᛖ ᛒᚢᛞᛖ ᚩᚾ ᚦᚫᛗ ᛚᚪᚾᛞᛖ ᚾᚩᚱᚦᚹᛖᚪᚱᛞᚢᛗ ᚹᛁᚦ ᚦᚪ ᚹᛖᛥᚫ";
    final String braille = "⡌⠁⠧⠑ ⠼⠁⠒  ⡍⠜⠇⠑⠹⠰⠎ ⡣⠕⠌";
    System.out.println(maths);
    System.out.println(runes);
    System.out.println(braille);
  }
}
