/* UZ: JNI o'rniga bo'sh e'lonlar (IntroRenderer.c ni Android'siz yig'ish uchun). EN: Empty JNI declarations so IntroRenderer.c builds without Android. RU: Пустые объявления JNI, чтобы собрать IntroRenderer.c без Android. */
#pragma once
#include <stdint.h>
typedef void JNIEnv;
typedef void *jclass;
typedef float jfloat;
typedef int32_t jint;
#define JNIEXPORT
#define JNICALL
