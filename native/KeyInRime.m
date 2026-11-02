#import "KeyInRime.h"
#include <rime_api.h>
static RimeApi *api;
static BOOL started;
@implementation KeyInRime {
    RimeSessionId _session;
    BOOL _learning;
}
+ (BOOL)startWithSharedData:(NSString *)shared userData:(NSString *)user {
    if (started) return YES;
    if (![NSFileManager.defaultManager fileExistsAtPath:[shared stringByAppendingPathComponent:@"build/keyin_pinyin.schema.yaml"]]) return NO;
    NSError *error;
    if (![NSFileManager.defaultManager createDirectoryAtPath:user withIntermediateDirectories:YES attributes:@{NSFilePosixPermissions: @0700} error:&error]) return NO;
    api = rime_get_api();
    RIME_STRUCT(RimeTraits, traits);
    traits.shared_data_dir = shared.fileSystemRepresentation;
    traits.user_data_dir = user.fileSystemRepresentation;
    traits.distribution_name = "KeyIn";
    traits.distribution_code_name = "keyin";
    traits.distribution_version = "0.2.0";
    traits.app_name = "rime.keyin";
    traits.min_log_level = 2;
    traits.log_dir = "";
    static const char *modules[] = {"default", NULL};
    traits.modules = modules;
    api->setup(&traits);
    api->initialize(&traits);
    started = YES;
    return YES;
}
+ (NSString *)version { return started ? [NSString stringWithUTF8String:api->get_version()] : @"unavailable"; }
- (instancetype)init {
    self = [super init];
    _learning = YES;
    if (self && started) {
        _session = api->create_session();
        if (_session && !api->select_schema(_session, "keyin_pinyin")) {
            api->destroy_session(_session); _session = 0;
        }
        if (_session) api->set_option(_session, "ascii_mode", False);
    }
    return self;
}
- (void)dealloc { if (_session) api->destroy_session(_session); }
- (BOOL)learning { return _learning; }
- (void)setLearning:(BOOL)learning {
    if (_learning == learning || !_session) return;
    [self clear];
    if (api->select_schema(_session, learning ? "keyin_pinyin" : "keyin_pinyin_private")) _learning = learning;
}
- (BOOL)available { return _session != 0; }
- (NSString *)input {
    const char *input = _session ? api->get_input(_session) : NULL;
    return input ? [NSString stringWithUTF8String:input] : @"";
}
- (NSUInteger)caret { return _session ? api->get_caret_pos(_session) : 0; }
- (void)clear { if (_session) api->clear_composition(_session); }
- (BOOL)processKey:(int)key { return _session && api->process_key(_session, key, 0); }
- (BOOL)setInput:(NSString *)raw {
    if (!_session || raw.length > 128) return NO;
    NSString *normalized = raw.lowercaseString;
    NSString *old = self.input;
    if ([old isEqualToString:normalized]) return YES;
    NSUInteger start = 0;
    if ([normalized hasPrefix:old] && self.caret == old.length) start = old.length;
    else [self clear];
    for (NSUInteger i = start; i < normalized.length; i++) {
        unichar key = [normalized characterAtIndex:i];
        if (!((key >= 'a' && key <= 'z') || key == '\'')) { [self clear]; return NO; }
        if (![self processKey:key]) { [self clear]; return NO; }
    }
    return YES;
}
- (NSDictionary *)snapshot {
    if (!_session) return nil;
    RIME_STRUCT(RimeContext, ctx);
    if (!api->get_context(_session, &ctx)) return nil;
    NSString *preedit = ctx.composition.preedit ? [NSString stringWithUTF8String:ctx.composition.preedit] : @"";
    // Rime offsets are UTF-8 bytes; InputMethodKit expects UTF-16 positions.
    NSData *bytes = [preedit dataUsingEncoding:NSUTF8StringEncoding];
    NSUInteger cursorBytes = MIN((NSUInteger)MAX(0, ctx.composition.cursor_pos), bytes.length);
    NSString *prefix = [[NSString alloc] initWithData:[bytes subdataWithRange:NSMakeRange(0, cursorBytes)] encoding:NSUTF8StringEncoding];
    NSUInteger cursor = prefix ? prefix.length : preedit.length;
    api->free_context(&ctx);
    NSMutableArray *choices = [NSMutableArray new];
    RimeCandidateListIterator iterator = {0};
    if (api->candidate_list_begin(_session, &iterator)) {
        while (choices.count < 90 && api->candidate_list_next(&iterator)) {
            if (iterator.candidate.text) [choices addObject:[NSString stringWithUTF8String:iterator.candidate.text]];
        }
        api->candidate_list_end(&iterator);
    }
    return @{@"preedit": preedit, @"caret": @(cursor), @"choices": choices};
}
- (NSString *)selectCandidate:(NSUInteger)index {
    if (!_session || !api->select_candidate(_session, index)) return nil;
    RIME_STRUCT(RimeCommit, commit);
    if (!api->get_commit(_session, &commit)) return nil; // Selected one segment; continue composing.
    NSString *text = commit.text ? [NSString stringWithUTF8String:commit.text] : @"";
    api->free_commit(&commit);
    return text;
}
@end
