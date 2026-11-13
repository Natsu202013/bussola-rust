# No obfuscation is applied (isMinifyEnabled = false), these rules only document
# the JNI surface in case shrinking is enabled later.
-keepclasseswithmembernames class com.bussola.core.Core {
    native <methods>;
}
-keep class com.bussola.core.Core { *; }
