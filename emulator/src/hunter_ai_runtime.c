/* SPDX-License-Identifier: MPL-2.0 */
#include "quickjs.h"
#include <stdint.h>
#include <stdlib.h>

typedef struct {
    JSRuntime *runtime;
    JSContext *context;
    unsigned ticks;
} HunterAi;

static int interrupt(JSRuntime *runtime, void *opaque) {
    (void)runtime;
    HunterAi *ai = opaque;
    return ++ai->ticks >= 4096;
}

void hunter_ai_free(HunterAi *ai) {
    if (!ai) return;
    if (ai->context) JS_FreeContext(ai->context);
    if (ai->runtime) JS_FreeRuntime(ai->runtime);
    free(ai);
}

HunterAi *hunter_ai_new(const char *script, size_t length) {
    HunterAi *ai = calloc(1, sizeof(*ai));
    if (!ai) return NULL;
    ai->runtime = JS_NewRuntime();
    if (!ai->runtime) { hunter_ai_free(ai); return NULL; }
    JS_SetMemoryLimit(ai->runtime, 64 * 1024 * 1024);
    JS_SetMaxStackSize(ai->runtime, 1024 * 1024);
    JS_SetInterruptHandler(ai->runtime, interrupt, ai);
    ai->context = JS_NewContext(ai->runtime);
    if (!ai->context) { hunter_ai_free(ai); return NULL; }
    JSValue result = JS_Eval(ai->context, script, length, "hunter-hardmode-v2.js", JS_EVAL_TYPE_GLOBAL);
    const int failed = JS_IsException(result);
    JS_FreeValue(ai->context, result);
    if (failed) { hunter_ai_free(ai); return NULL; }
    return ai;
}

/* 1 = successful reply, 0 = exception text. Both are released with hunter_ai_free_text. */
const char *hunter_ai_call(HunterAi *ai, const char *method, const char *json, size_t length, int *ok) {
    ai->ticks = 0;
    JSContext *ctx = ai->context;
    JSValue global = JS_GetGlobalObject(ctx);
    JSValue function = JS_GetPropertyStr(ctx, global, method);
    JSValue argument = JS_NewStringLen(ctx, json, length);
    JSValue result = JS_IsException(argument) ? JS_EXCEPTION : JS_Call(ctx, function, global, 1, &argument);
    *ok = !JS_IsException(result);
    if (!*ok) result = JS_GetException(ctx);
    const char *text = JS_ToCString(ctx, result);
    JS_FreeValue(ctx, result);
    JS_FreeValue(ctx, argument);
    JS_FreeValue(ctx, function);
    JS_FreeValue(ctx, global);
    return text;
}

void hunter_ai_free_text(HunterAi *ai, const char *text) {
    JS_FreeCString(ai->context, text);
}
