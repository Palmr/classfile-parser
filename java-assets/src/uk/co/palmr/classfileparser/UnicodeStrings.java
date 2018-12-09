package uk.co.palmr.classfileparser;

public class UnicodeStrings {
  public static void main(String[] args) {
    final String maths = "2H₂ + O₂ ⇌ 2H₂O, R = 4.7 kΩ, ⌀ 200 mm";
    final String runes = "ᚻᛖ ᚳᚹᚫᚦ ᚦᚫᛏ ᚻᛖ ᛒᚢᛞᛖ ᚩᚾ ᚦᚫᛗ ᛚᚪᚾᛞᛖ ᚾᚩᚱᚦᚹᛖᚪᚱᛞᚢᛗ ᚹᛁᚦ ᚦᚪ ᚹᛖᛥᚫ";
    final String braille = "⡌⠁⠧⠑ ⠼⠁⠒  ⡍⠜⠇⠑⠹⠰⠎ ⡣⠕⠌";
    final String modified = "\0𠜎";
    final String unpaired = "X\uD800X";
    System.out.println(maths);
    System.out.println(runes);
    System.out.println(braille);
    System.out.println(modified);
    System.out.println(unpaired);
  }
}
