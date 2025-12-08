#!/bin/sh

if [ -n "${JAVA_HOME}" ] ; then
    JAVAC=${JAVA_HOME}/bin/javac
    if [ ! -x "${JAVAC}" ] ; then
        echo "ERROR: JAVA_HOME is set to an invalid directory: ${JAVA_HOME}

Please set the JAVA_HOME variable in your environment to match the
location of your Java installation."
        exit 1
    fi
else
    JAVAC=javac
    which javac >/dev/null 2>&1 || echo "ERROR: JAVA_HOME is not set and no 'javac' command could be found in your PATH.

Please set the JAVA_HOME variable in your environment to match the
location of your Java installation." && exit 1
fi

echo "Using: $(${JAVAC} -version)"

javac -d java-assets/compiled-classes/ -parameters -g java-assets/src/BasicClass.java
javac -d java-assets/compiled-classes/ java-assets/src/BootstrapMethods.java
javac -d java-assets/compiled-classes/ -g:none java-assets/src/Factorial.java
javac -d java-assets/compiled-classes/ java-assets/src/Instructions.java
javac -d java-assets/compiled-classes/ java-assets/src/UnicodeStrings.java
javac -d java-assets/compiled-classes/ java-assets/src/DeprecatedAnnotation.java
javac -d java-assets/compiled-classes/ java-assets/src/InnerClasses.java
javac -d java-assets/compiled-classes/ java-assets/src/Annotations.java

javac -g -d java-assets/compiled-classes/ java-assets/src/LocalVariableTable.java
javac -d java-assets/compiled-classes/ java-assets/src/HelloWorld.java
printf '\xde\xad\xbe\xef' > java-assets/compiled-classes/malformed.class
tail -c+5 java-assets/compiled-classes/HelloWorld.class >> java-assets/compiled-classes/malformed.class

