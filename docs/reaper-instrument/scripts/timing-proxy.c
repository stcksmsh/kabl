/* Measurement-only CLAP proxy. Never install this instead of the product.
 * ABI fields follow clap-sys 0.5.0 / CLAP plugin, entry and factory declarations.
 * Fixed storage; callback does no allocation, logging or locking.
 * Each timed process call includes two CLOCK_MONOTONIC reads and atomic updates.
 */
#include <stdint.h>
#include <stdbool.h>
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <dlfcn.h>
struct plugin;
struct version { uint32_t major, minor, revision; };
struct plugin {
 const void *desc; void *data;
 bool (*init)(const struct plugin *);
 void (*destroy)(const struct plugin *);
 bool (*activate)(const struct plugin *,double,uint32_t,uint32_t);
 void (*deactivate)(const struct plugin *);
 bool (*start)(const struct plugin *);
 void (*stop)(const struct plugin *);
 void (*reset)(const struct plugin *);
 int32_t (*process)(const struct plugin *,const void *);
 const void *(*extension)(const struct plugin *,const char *);
 void (*main)(const struct plugin *);
};
struct factory {
 uint32_t (*count)(const struct factory *);
 const void *(*descriptor)(const struct factory *,uint32_t);
 const struct plugin *(*create)(const struct factory *,const void *,const char *);
};
struct entry { struct version version; bool (*init)(const char *); void (*deinit)(void); const void *(*factory)(const char *); };
struct event_header {uint32_t size,time;uint16_t space,type;uint32_t flags;};
struct input_events {void *ctx;uint32_t (*size)(const struct input_events *);const struct event_header *(*get)(const struct input_events *,uint32_t);};
struct process_info {int64_t steady;uint32_t frames;const void *transport,*inputs,*outputs;uint32_t input_count,output_count;const struct input_events *events;const void *out_events;};
static const struct entry *real_entry;
static const struct factory *real_factory;
struct slot {
 const struct plugin *plugin;
 int32_t (*process)(const struct plugin *,const void *);
 void (*destroy)(const struct plugin *);
 atomic_uint_fast64_t count, total, maximum, histogram[1000], frames;
 void (*reset)(const struct plugin *); bool (*start)(const struct plugin *);
 struct {unsigned kind;uint64_t frame;int64_t steady;} trace[64]; atomic_uint trace_count;
};
static struct slot slots[16];
static unsigned used; static bool reset_trace;
static uint64_t now(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC,&t); return (uint64_t)t.tv_sec*1000000000+t.tv_nsec; }
static struct slot *find(const struct plugin *p) { for(unsigned i=0;i<used;i++) if(slots[i].plugin==p) return &slots[i]; abort(); }
static void trace(struct slot *s,unsigned kind,int64_t steady) {
 unsigned i=atomic_fetch_add(&s->trace_count,1);if(i<64){s->trace[i].kind=kind;s->trace[i].frame=atomic_load(&s->frames);s->trace[i].steady=steady;}
}
static void reset(const struct plugin *p){struct slot *s=find(p);trace(s,1,0);s->reset(p);}
static bool start(const struct plugin *p){struct slot *s=find(p);trace(s,2,0);return s->start(p);}
static int32_t timed(const struct plugin *p,const void *input) {
 struct slot *s=find(p);
 if(reset_trace) {
  const struct process_info *info=input;
  if(info->events&&info->events->size&&info->events->get) {
   uint32_t n=info->events->size(info->events);
   for(uint32_t i=0;i<n&&i<2048;i++) {
    const struct event_header *h=info->events->get(info->events,i);
    if(h&&h->space==0&&h->type==10&&h->size>=21) {
     const unsigned char *bytes=(const unsigned char *)h;
     if((bytes[18]&0xf0)==0x90&&bytes[20]) {trace(s,3,info->steady+h->time);break;}
    }
   }
  }
  atomic_fetch_add(&s->frames,info->frames);
 }
 uint64_t start=now(); int32_t result=s->process(p,input); uint64_t ns=now()-start;
 atomic_fetch_add_explicit(&s->count,1,memory_order_relaxed);
 atomic_fetch_add_explicit(&s->total,ns,memory_order_relaxed);
 uint64_t old=atomic_load_explicit(&s->maximum,memory_order_relaxed);
 if(ns>old) atomic_store_explicit(&s->maximum,ns,memory_order_relaxed); /* host serializes instance callbacks */
 atomic_fetch_add_explicit(&s->histogram[ns/10000<999?ns/10000:999],1,memory_order_relaxed);
 return result;
}
static void destroy(const struct plugin *p) {
 struct slot *s=find(p);
 const char *path=getenv("KABL_TIMING_OUTPUT"); FILE *f=path?fopen(path,"a"):NULL;
 if(f) {
  uint64_t n=atomic_load(&s->count),sum=atomic_load(&s->total),max=atomic_load(&s->maximum), cumulative=0,p99=0;
  for(unsigned i=0;i<1000;i++){ cumulative+=atomic_load(&s->histogram[i]); if(n&&cumulative>=(n*99+99)/100){p99=(i+1)*10000;break;} }
  fprintf(f,"%u,%lu,%lu,%lu,%lu\n",(unsigned)(s-slots),(unsigned long)n,(unsigned long)sum,(unsigned long)max,(unsigned long)p99); fclose(f);
  if(reset_trace){char name[4096];snprintf(name,sizeof name,"%s.trace",path);f=fopen(name,"a");if(f){unsigned n=atomic_load(&s->trace_count);for(unsigned i=0;i<n&&i<64;i++)fprintf(f,"%u,%u,%lu,%ld\n",(unsigned)(s-slots),s->trace[i].kind,(unsigned long)s->trace[i].frame,(long)s->trace[i].steady);fclose(f);}}
 }
 s->destroy(p); s->plugin=NULL;
}
static uint32_t count(const struct factory *f) { (void)f; return real_factory->count(real_factory); }
static const void *descriptor(const struct factory *f,uint32_t i) { (void)f;return real_factory->descriptor(real_factory,i); }
static const struct plugin *create(const struct factory *f,const void *host,const char *id) {
 (void)f; if(used==16) return NULL;
 const struct plugin *p=real_factory->create(real_factory,host,id); if(!p)return NULL;
 struct slot *s=&slots[used++];s->plugin=p;s->process=p->process;s->destroy=p->destroy;s->reset=p->reset;s->start=p->start;
 ((struct plugin *)p)->reset=reset;((struct plugin *)p)->start=start;((struct plugin *)p)->process=timed;((struct plugin *)p)->destroy=destroy;return p;
}
static struct factory wrapped={count,descriptor,create};
static bool init(const char *path) {
 reset_trace=getenv("KABL_RESET_TRACE")!=NULL; const char *target=getenv("KABL_TIMING_PLUGIN");if(!target)return false;
 void *lib=dlopen(target,RTLD_NOW|RTLD_LOCAL);if(!lib)return false;
 real_entry=dlsym(lib,"clap_entry");return real_entry&&real_entry->init(path);
}
static void deinit(void) { if(real_entry)real_entry->deinit(); }
static const void *factory(const char *id) {
 const void *f=real_entry->factory(id);
 if(f&&!strcmp(id,"clap.plugin-factory")){real_factory=f;return &wrapped;}return f;
}
__attribute__((visibility("default"))) const struct entry clap_entry={{1,2,2},init,deinit,factory};
