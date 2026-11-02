#import <Foundation/Foundation.h>
#import "KeyInRime.h"
static void check(BOOL ok, NSString *message) {
    if (!ok) { fprintf(stderr, "FAIL: %s\n", message.UTF8String); exit(1); }
}
int main(int argc, const char **argv) { @autoreleasepool {
    check(argc == 3, @"shared and user data arguments");
    check([KeyInRime startWithSharedData:@(argv[1]) userData:@(argv[2])], @"startup");
    KeyInRime *engine = [KeyInRime new];
    check(engine.available, @"session/schema");
    [engine setInput:@"nhlm"];
    check([[[engine snapshot][@"choices"] firstObject] isEqualToString:@"你好了吗"], @"initials with fresh user dictionary");
    [engine clear];
    NSDictionary *examples = @{@"kankan": @"看看", @"zenmeyang": @"怎么样", @"nihao": @"你好", @"nihaolema": @"你好了吗", @"woganjuehenhaole": @"我感觉很好了"};
    for (NSString *raw in examples) {
        [engine clear];
        check([engine setInput:raw], raw);
        NSDictionary *snapshot = [engine snapshot];
        NSArray *choices = snapshot[@"choices"];
        printf("%s -> %s / %s\n", raw.UTF8String, [snapshot[@"preedit"] UTF8String], [[choices firstObject] UTF8String]);
        check([choices containsObject:examples[raw]], [@"candidate: " stringByAppendingString:raw]);
        NSUInteger index = [choices indexOfObject:examples[raw]];
        check([[engine selectCandidate:index] isEqualToString:examples[raw]], @"commit selected candidate");
    }
    [engine clear];
    check([engine setInput:@"ni'hao"], @"apostrophe");
    check([[[engine snapshot][@"choices"] firstObject] isEqualToString:@"你好"], @"apostrophe candidates");
    check([engine processKey:0xff08], @"backspace");
    check([engine.input isEqualToString:@"ni'ha"], @"raw editing");
    [engine clear]; [engine setInput:@"nihao"];
    [engine processKey:0xff51]; // left
    printf("caret after left: %lu\n", (unsigned long)engine.caret);
    check(engine.caret < 5, @"caret movement");
    [engine processKey:0xff08];
    printf("after delete: %s\n", engine.input.UTF8String);
    check(engine.input.length == 4, @"delete in middle");
    [engine clear]; [engine setInput:@"nihao"];
    NSArray *segmentChoices = [engine snapshot][@"choices"];
    NSUInteger ni = [segmentChoices indexOfObject:@"你"];
    check(ni != NSNotFound, @"partial segment candidate");
    check([engine selectCandidate:ni] == nil, @"partial selection does not prematurely commit");
    check([[engine snapshot][@"preedit"] hasPrefix:@"你"], @"selected segment remains in preedit");
    check([[engine selectCandidate:0] isEqualToString:@"你好"], @"remaining segment commits full phrase");
    engine.learning = NO;
    check(!engine.learning, @"disable local learning");
    [engine setInput:@"zenmeyang"];
    check([[[engine snapshot][@"choices"] firstObject] isEqualToString:@"怎么样"], @"private schema works");
    [engine clear]; engine.learning = YES;
    check(engine.learning, @"reenable local learning");
    NSMutableArray<NSNumber *> *timings = [NSMutableArray new];
    NSString *sentence = @"woganjuehenhaole";
    for (NSUInteger repeat = 0; repeat < 20; repeat++) {
        [engine clear];
        for (NSUInteger length = 1; length <= sentence.length; length++) {
            NSTimeInterval start = NSProcessInfo.processInfo.systemUptime;
            [engine setInput:[sentence substringToIndex:length]];
            [engine snapshot];
            [timings addObject:@((NSProcessInfo.processInfo.systemUptime - start) * 1000)];
        }
    }
    [timings sortUsingSelector:@selector(compare:)];
    printf("Warm Rime input+snapshot p95 %.2f ms, max %.2f ms (%lu samples)\n", timings[timings.count * 95 / 100].doubleValue, timings.lastObject.doubleValue, (unsigned long)timings.count);
    KeyInRime *other = [KeyInRime new];
    check(other.input.length == 0, @"isolated sessions");
    puts("Rime integration smoke checks passed");
    return 0;
}}
