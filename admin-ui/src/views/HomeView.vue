<script setup lang="ts">
import { ref, watch } from 'vue';
import { useFetch } from '@vueuse/core';
// import { useCounterStore } from '@/stores/counter';

interface StickerSetPub {
  id: string,
  title?: string,
}

const { data, error, execute: refetch } = useFetch('/api/pending-sets', { refetch: true, updateDataOnError: true }).json<StickerSetPub[]>()

watch(data, () => console.log(data))

// const counter = useCounterStore();

</script>

<template>
  <main>
    <div class="d-flex">
      <div>
        <!-- <span @click="counter.increment()">
          {{ counter.count }}
        </span> -->
        <div v-if="data">
          <div v-for="set of data">
            <v-btn variant="plain" width="auto" height="auto" :to="{ name: 'banSetView', params: { setId: set.id } }">
                <img :src="`/thumbnails/sticker-set/${set.id}/image.png`" loading="lazy" width="128" height="128" />
              </v-btn>
                <span>
                  {{ set.id }}
                </span>
          </div>

        </div>
        <v-btn color="primary" @click="refetch()">
          refresh
        </v-btn>
        {{ error }}
      </div>

      <router-view />
    </div>
  </main>
</template>
