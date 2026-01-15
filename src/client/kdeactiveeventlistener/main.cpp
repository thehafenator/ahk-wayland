#include "activeclienteffect.h"
#include <kwin/effect/effect.h>

KWIN_EFFECT_FACTORY_SUPPORTED(KWin::ActiveClientEffect,
                               "metadata.json",
                               return KWin::ActiveClientEffect::supported();)

#include "main.moc"