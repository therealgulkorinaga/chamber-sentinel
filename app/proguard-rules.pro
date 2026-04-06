# Chamber Sentinel ProGuard rules

# Keep JNI bridge methods
-keep class com.chamber.sentinel.ChamberBridge {
    native <methods>;
    public static *;
}

# Keep all native method names
-keepclasseswithmembernames class * {
    native <methods>;
}
